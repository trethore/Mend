use clap::Parser;
use is_terminal::IsTerminal;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

mod diff;
mod parser;
mod patcher;
use patcher::{FilePatchResult, PatchError};

#[derive(Parser, Debug)]
#[command(
    author = "Tytoo",
    version,
    about = "A fuzzy diff applicator",
    long_about = r#"
Mend is a command-line tool to apply llm generated diffs using fuzzy matching.

It can read a diff from a file or from standard input.

Examples:
  mend my_feature.diff
  git diff | mend
  cat some_change.patch | mend
"#
)]
struct Args {
    #[arg(index = 1)]
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
    io::stdin().read_line(&mut input).expect("Failed to read line");
    input.trim().to_string()
}

fn resolve_file_diff_interactively(
    file_diff: &diff::FileDiff,
    fuzziness: u8,
    debug_mode: bool,
    match_threshold: f32,
) -> Result<Option<FilePatchResult>, PatchError> {
    if file_diff.new_file == "/dev/null" {
        return Ok(Some(FilePatchResult::Deleted {
            path: file_diff.old_file.clone(),
        }));
    }

    let mut source_lines: Vec<String> = if file_diff.old_file == "/dev/null" {
        Vec::new()
    } else {
        fs::read_to_string(&file_diff.old_file)?
            .lines()
            .map(String::from)
            .collect()
    };

    for (i, hunk) in file_diff.hunks.iter().enumerate() {
        loop {
            let possible_matches = patcher::find_hunk_location(
                &source_lines,
                hunk,
                fuzziness,
                debug_mode,
                match_threshold,
            );

            if possible_matches.is_empty() {
                eprintln!("[ERROR] Failed to apply hunk {} for file {}. No matching context found.", i + 1, file_diff.new_file);
                eprintln!("Do you want to [s]kip this hunk or [a]bort the process? (s/a)");
                let choice = read_user_input();
                if choice.to_lowercase() == "s" {
                    break;
                } else if choice.to_lowercase() == "a" {
                    return Err(PatchError::HunkApplicationFailed {
                        file_path: file_diff.new_file.clone(),
                        hunk_index: i,
                        reason: "User aborted due to unresolvable hunk.".to_string(),
                    });
                } else {
                    eprintln!("Invalid choice. Please enter 's' to skip or 'a' to abort.");
                    continue;
                }
            } else if possible_matches.len() > 1 {
                eprintln!("[ERROR] Ambiguous match for hunk {} in file {}. Possible locations:", i + 1, file_diff.new_file);
                for (idx, m) in possible_matches.iter().enumerate() {
                    eprintln!("  {}. Line {} (Score: {:.2})", idx + 1, m.start_index + 1, m.score);
                }
                eprintln!("Enter the index of the correct location, [s]kip this hunk, or [a]bort: ");
                let choice = read_user_input();
                if choice.to_lowercase() == "s" {
                    break;
                } else if choice.to_lowercase() == "a" {
                    return Err(PatchError::AmbiguousMatch {
                        file_path: file_diff.new_file.clone(),
                        hunk_index: i,
                    });
                } else if let Ok(index) = choice.parse::<usize>() {
                    if index > 0 && index <= possible_matches.len() {
                        let chosen_match = &possible_matches[index - 1];
                        if debug_mode {
                            println!(
                                "[DEBUG] Hunk {}/{} matched at line {} (length {} lines)",
                                i + 1,
                                file_diff.hunks.len(),
                                chosen_match.start_index + 1,
                                chosen_match.matched_length
                            );
                        }
                        source_lines = patcher::apply_hunk(
                            &source_lines,
                            hunk,
                            chosen_match.start_index,
                            chosen_match.matched_length,
                        );
                        break;
                    } else {
                        eprintln!("Invalid index. Please enter a valid number, 's', or 'a'.");
                        continue;
                    }
                } else {
                    eprintln!("Invalid choice. Please enter a valid number, 's', or 'a'.");
                    continue;
                }
            } else {
                let chosen_match = &possible_matches[0];
                if debug_mode {
                    println!(
                        "[DEBUG] Hunk {}/{} matched at line {} (length {} lines)",
                        i + 1,
                        file_diff.hunks.len(),
                        chosen_match.start_index + 1,
                        chosen_match.matched_length
                    );
                }
                source_lines = patcher::apply_hunk(
                    &source_lines,
                    hunk,
                    chosen_match.start_index,
                    chosen_match.matched_length,
                );
                break;
            }
        }
    }

    let new_content = source_lines.join("\n");

    if file_diff.old_file == "/dev/null" {
        Ok(Some(FilePatchResult::Created {
            path: file_diff.new_file.clone(),
            new_content,
        }))
    } else {
        Ok(Some(FilePatchResult::Modified {
            path: file_diff.new_file.clone(),
            new_content,
        }))
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let is_verbose = args.verbose || args.debug;

    if is_verbose {
        println!("mend: A fuzzy diff applicator");
        println!("----------------------------");
    }

    let diff_content = match args.diff_file {
        Some(path) => {
            if is_verbose {
                println!("[INFO] Reading diff from file: {}", path);
            }
            fs::read_to_string(path)?
        }
        None => {
            if io::stdin().is_terminal() {
                eprintln!("[ERROR] No diff file specified and stdin is a terminal.");
                eprintln!("Usage: mend <DIFF_FILE>");
                eprintln!("Or pipe from stdin: git diff | mend");
                std::process::exit(1);
            }
            if is_verbose {
                println!("[INFO] Reading diff from stdin...");
            }
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
        println!(
            "[INFO] Parsed patch with {} file diff(s).",
            patch.diffs.len()
        );
        println!(
            "[INFO] Applying patches with fuzziness level {}.",
            args.fuzziness
        );
    }

    let mut all_patch_results: Vec<FilePatchResult> = Vec::new();

    for (i, file_diff) in patch.diffs.iter().enumerate() {
        if is_verbose {
            println!("[INFO] Processing file diff {}/{} for file {}", i + 1, patch.diffs.len(), file_diff.new_file);
        }
        match resolve_file_diff_interactively(file_diff, args.fuzziness, args.debug, args.match_threshold) {
            Ok(Some(result)) => {
                all_patch_results.push(result);
            }
            Ok(None) => {
                // do nothing.
            }
            Err(e) => {
                eprintln!("[ERROR] Could not apply patch: {}", e);
                std::process::exit(1);
            }
        }
    }

    if args.dry_run {
        println!("\n[DRY RUN] The following changes would be applied:");
        for result in all_patch_results {
            match result {
                FilePatchResult::Modified { path, .. } => {
                    println!("  - [MODIFIED] {}", path);
                }
                FilePatchResult::Created { path, .. } => {
                    println!("  - [CREATED]  {}", path);
                }
                FilePatchResult::Deleted { path } => {
                    println!("  - [DELETED]  {}", path);
                }
            }
        }
    } else {
        for result in all_patch_results {
            match result {
                FilePatchResult::Modified { path, new_content } => {
                    if let Err(e) = fs::write(&path, new_content) {
                        eprintln!("[ERROR] Failed to write to file {}: {}", path, e);
                        std::process::exit(1);
                    }
                }
                FilePatchResult::Created { path, new_content } => {
                    if let Some(parent) = Path::new(&path).parent() {
                        if !parent.exists() {
                            if let Err(e) = fs::create_dir_all(parent) {
                                eprintln!("[ERROR] Failed to create directory {}: {}", parent.display(), e);
                                std::process::exit(1);
                            }
                        }
                    }
                    if let Err(e) = fs::write(&path, new_content) {
                        eprintln!("[ERROR] Failed to create file {}: {}", path, e);
                        std::process::exit(1);
                    }
                }
                FilePatchResult::Deleted { path } => {
                    if let Err(e) = fs::remove_file(&path) {
                        eprintln!("[ERROR] Failed to delete file {}: {}", path, e);
                        std::process::exit(1);
                    }
                }
            }
        }
        println!("Successfully applied patch.");
    }

    if is_verbose {
        println!("----------------------------");
        println!("Execution finished.");
    }

    Ok(())
}