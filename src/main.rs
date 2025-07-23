use clap::Parser;
use is_terminal::IsTerminal;
use std::cmp::min;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

mod diff;
mod parser;
mod patcher;
use diff::FileDiff;
use patcher::{FilePatchResult, PatchError};

#[derive(Parser, Debug)]
#[command(
    author = "Tytoo",
    version,
    about = "A fuzzy diff applicator",
    long_about = r#"
Mend is a command-line tool to apply llm generated diffs using fuzzy matching.

It can read a diff from a file or from standard input.

# Apply a patch, auto-detecting the target file from diff headers
mend my_feature.diff

# Explicitly provide the target file, ignoring diff headers
mend src/main.rs my_changes.diff

# Pipe a diff from stdin and apply to an explicit target file
git diff | mend src/main.rs
"#
)]
struct Args {
    target_file: Option<String>,

    diff_file: Option<String>,

    #[arg(long)]
    debug: bool,

    #[arg(long, default_value_t = false)]
    dry_run: bool,

    #[arg(short, long, default_value_t = 2)]
    fuzziness: u8,

    #[arg(short = 'm', long, default_value_t = 0.7)]
    match_threshold: f32,

    #[arg(short, long)]
    verbose: bool,
}

fn read_user_input() -> String {
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");
    input.trim().to_string()
}
fn is_binary(path: &Path) -> io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut buffer = [0; 1024];
    let n = file.read(&mut buffer)?;
    Ok(buffer[..n].contains(&0))
}
fn print_match_context(
    source_lines: &[String],
    hunk_match: &patcher::HunkMatch,
    option_index: usize,
) {
    const CONTEXT_LINES: usize = 2;
    eprintln!(
        "\n> Option {} (Line {}, Score: {:.2})",
        option_index,
        hunk_match.start_index + 1,
        hunk_match.score
    );
    let start_line = hunk_match.start_index;
    let context_before_start = start_line.saturating_sub(CONTEXT_LINES);
    for i in context_before_start..start_line {
        eprintln!("  {:>4} | {}", i + 1, source_lines[i]);
    }
    eprintln!(
        "  ---- | --- (Patch would be applied here, replacing {} lines) ---",
        hunk_match.matched_length
    );
    let end_line = start_line + hunk_match.matched_length;
    let context_after_end = min(source_lines.len(), end_line + CONTEXT_LINES);
    for i in end_line..context_after_end {
        if i < source_lines.len() {
            eprintln!("  {:>4} | {}", i + 1, source_lines[i]);
        }
    }
}


fn resolve_file_diff_interactively(
    file_diff: &FileDiff,
    cli_target_path: &Option<String>,
    fuzziness: u8,
    debug_mode: bool,
    match_threshold: f32,
) -> Result<Option<FilePatchResult>, PatchError> {
    let old_path = cli_target_path.clone().unwrap_or_else(|| file_diff.old_file.clone());
    let new_path = cli_target_path.clone().unwrap_or_else(|| file_diff.new_file.clone());

    if old_path.is_empty() && new_path != "/dev/null" {
        return Err(PatchError::IOError("Could not determine target file. The diff has no file headers. Please specify the target file: `mend <TARGET_FILE> [DIFF_FILE]`".to_string()));
    }

    if new_path == "/dev/null" {
        return Ok(Some(FilePatchResult::Deleted { path: old_path }));
    }

    let mut source_lines: Vec<String> = if old_path == "/dev/null" {
        Vec::new()
    } else {
        let path = Path::new(&old_path);
        if !path.exists() {
             return Err(PatchError::IOError(format!("Original file not found: {}", path.display())));
        }
        if is_binary(path).unwrap_or(false) {
            eprintln!("[WARN] Skipping binary file: {}", old_path);
            return Ok(None);
        }
        fs::read_to_string(path)?
            .lines()
            .map(String::from)
            .collect()
    };

    for (i, hunk) in file_diff.hunks.iter().enumerate().rev() {
        loop {
            let possible_matches = patcher::find_hunk_location(
                &source_lines,
                hunk,
                fuzziness,
                debug_mode,
                match_threshold,
            );
            if possible_matches.is_empty() {
                eprintln!("[ERROR] Failed to apply hunk {} for file {}. No matching context found.", i + 1, new_path);
                eprintln!("Do you want to [s]kip this hunk or [a]bort the process? (s/a)");
                let choice = read_user_input();
                if choice.to_lowercase() == "s" { break; }
                else if choice.to_lowercase() == "a" {
                    return Err(PatchError::HunkApplicationFailed {
                        file_path: new_path.clone(),
                        hunk_index: i,
                        reason: "User aborted due to unresolvable hunk.".to_string(),
                    });
                } else {
                    eprintln!("Invalid choice. Please enter 's' to skip or 'a' to abort.");
                    continue;
                }
            } else if possible_matches.len() > 1 {
                eprintln!("[ERROR] Ambiguous match for hunk {} in file {}. Possible locations:", i + 1, new_path);
                for (idx, m) in possible_matches.iter().enumerate() {
                    print_match_context(&source_lines, m, idx + 1);
                }
                eprintln!("\nEnter the index of the correct location, [s]kip this hunk, or [a]bort: ");
                let choice = read_user_input();
                if choice.to_lowercase() == "s" { break; }
                else if choice.to_lowercase() == "a" {
                    return Err(PatchError::AmbiguousMatch { file_path: new_path.clone(), hunk_index: i });
                } else if let Ok(index) = choice.parse::<usize>() {
                    if index > 0 && index <= possible_matches.len() {
                        let chosen_match = &possible_matches[index - 1];
                        source_lines = patcher::apply_hunk(&source_lines, hunk, chosen_match.start_index, chosen_match.matched_length);
                        break;
                    } else { eprintln!("Invalid index. Please enter a valid number, 's', or 'a'."); continue; }
                } else { eprintln!("Invalid choice. Please enter a valid number, 's', or 'a'."); continue; }
            } else {
                let chosen_match = &possible_matches[0];
                source_lines = patcher::apply_hunk(&source_lines, hunk, chosen_match.start_index, chosen_match.matched_length);
                break;
            }
        }
    }

    let new_content = source_lines.join("\n");

    if old_path == "/dev/null" {
        Ok(Some(FilePatchResult::Created { path: new_path, new_content }))
    } else {
        Ok(Some(FilePatchResult::Modified { path: new_path, new_content }))
    }
}

fn apply_changes(results: &[FilePatchResult]) -> io::Result<()> {
    for result in results {
        match result {
            FilePatchResult::Modified { path, new_content } => {
                fs::write(path, new_content)?;
            }
            FilePatchResult::Created { path, new_content } => {
                if let Some(parent) = Path::new(path).parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::write(path, new_content)?;
            }
            FilePatchResult::Deleted { path } => {
                fs::remove_file(path)?;
            }
        }
    }
    Ok(())
}


fn main() -> io::Result<()> {
    let mut args = Args::parse();
    let is_verbose = args.verbose || args.debug;

    if args.target_file.is_some() && args.diff_file.is_none() {
        args.diff_file = args.target_file.take();
    }

    if is_verbose {
        println!("mend: A fuzzy diff applicator");
        println!("----------------------------");
    }

    let diff_content = match args.diff_file {
        Some(path) => {
            if is_verbose { println!("[INFO] Reading diff from file: {}", path); }
            fs::read_to_string(path)?
        }
        None => {
            if io::stdin().is_terminal() {
                eprintln!("[ERROR] No diff file or stdin pipe detected.");
                eprintln!("Usage: mend [TARGET_FILE] <DIFF_FILE>");
                eprintln!("Or pipe from stdin: git diff | mend [TARGET_FILE]");
                std::process::exit(1);
            }
            if is_verbose { println!("[INFO] Reading diff from stdin..."); }
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            if buffer.is_empty() {
                eprintln!("[ERROR] Diff content from stdin was empty.");
                std::process::exit(1);
            }
            buffer
        }
    };

    let patch = match parser::parse_patch(&diff_content) {
        Ok(patch) => patch,
        Err(e) => {
            eprintln!("[ERROR] Failed to parse patch: {}", e);
            std::process::exit(1);
        }
    };
    if is_verbose {
        println!("[INFO] Parsed patch with {} file diff(s).", patch.diffs.len());
        println!("[INFO] Applying patches with fuzziness level {}.", args.fuzziness);
    }
    let mut all_patch_results: Vec<FilePatchResult> = Vec::new();
    for (i, file_diff) in patch.diffs.iter().enumerate() {
        if is_verbose {
            println!("[INFO] Processing file diff {}/{} for file '{}'", i + 1, patch.diffs.len(),
                args.target_file.as_deref().unwrap_or(&file_diff.new_file));
        }
        match resolve_file_diff_interactively(file_diff, &args.target_file, args.fuzziness, args.debug, args.match_threshold) {
            Ok(Some(result)) => { all_patch_results.push(result); }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[ERROR] Could not apply patch: {}", e);
                eprintln!("[INFO] No files were changed.");
                std::process::exit(1);
            }
        }
    }
    if args.dry_run || args.debug {
        println!("\n[DRY RUN] The following changes would be applied:");
        for result in all_patch_results {
            match result {
                FilePatchResult::Modified { path, .. } => println!("  - [MODIFIED] {}", path),
                FilePatchResult::Created { path, .. } => println!("  - [CREATED]  {}", path),
                FilePatchResult::Deleted { path } => println!("  - [DELETED]  {}", path),
            }
        }
    } else {
        if let Err(e) = apply_changes(&all_patch_results) {
            eprintln!("[ERROR] A failure occurred while writing changes to disk: {}", e);
            eprintln!("The patching process was aborted. Some files may have been changed.");
            std::process::exit(1);
        }
        println!("Successfully applied patch.");
    }
    if is_verbose {
        println!("----------------------------");
        println!("Execution finished.");
    }
    Ok(())
}