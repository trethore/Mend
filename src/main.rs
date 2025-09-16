use clap::Parser;
use is_terminal::IsTerminal;
use std::cmp::min;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clipboard::{ClipboardContext, ClipboardProvider};
use mend::error::AppError;

use mend::diff::{FileDiff, Patch};
use mend::parser;
use mend::patcher::{self, FilePatchResult, PatchError};
use std::time::Instant;
use std::{fs, process};

const EXAMPLE_DIFF: &str = include_str!("../resources/example.diff");

#[derive(Default, Debug)]
struct Report {
    files_modified: usize,
    files_created: usize,
    files_deleted: usize,
    hunks_applied: usize,
    hunks_skipped: usize,
    warnings: Vec<String>,
    elapsed_ms: Option<u128>,
}

impl Report {
    fn summary(&self, dry_run: bool, revert: bool) -> String {
        let action = if revert { "reverted" } else { "applied" };
        let time_str = self
            .elapsed_ms
            .map(|ms| {
                if ms < 1000 {
                    format!(" in {ms}ms")
                } else {
                    format!(" in {:.2}s", (ms as f64) / 1000.0)
                }
            })
            .unwrap_or_default();
        let header = if dry_run {
            format!("\nSummary{time_str}")
        } else if self.warnings.is_empty() {
            format!("✔ Patch {action} successfully{time_str}")
        } else {
            format!("✔ Patch {action} with warnings{time_str}")
        };

        let mut file_parts = Vec::new();
        if self.files_created > 0 {
            file_parts.push(format!("{} created", self.files_created));
        }
        if self.files_modified > 0 {
            file_parts.push(format!("{} modified", self.files_modified));
        }
        if self.files_deleted > 0 {
            file_parts.push(format!("{} deleted", self.files_deleted));
        }

        let mut hunk_parts = Vec::new();
        let hunk_text = if self.hunks_applied == 1 {
            "hunk"
        } else {
            "hunks"
        };
        hunk_parts.push(format!("{} {} {}", self.hunks_applied, hunk_text, action));
        if self.hunks_skipped > 0 {
            hunk_parts.push(format!("{} skipped", self.hunks_skipped));
        }

        let mut summary_parts = Vec::new();
        if !file_parts.is_empty() {
            summary_parts.push(file_parts.join(", "));
        }
        summary_parts.push(hunk_parts.join(", "));

        let summary_line = summary_parts.join(" | ");

        let mut final_string = format!("{header}: {summary_line}");

        if !self.warnings.is_empty() {
            final_string.push_str("\n\n--- Warnings ---");
            for warning in &self.warnings {
                final_string.push_str(&format!("\n- {warning}"));
            }
        }

        final_string
    }
}

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

# Read diff from clipboard and apply to an explicit target file
mend -c src/main.rs
"#
)]
struct Args {
    target_file: Option<String>,

    #[arg(conflicts_with = "clipboard")]
    diff_file: Option<String>,

    #[arg(short, long, conflicts_with = "diff_file")]
    clipboard: bool,

    #[arg(short, long, default_value_t = false)]
    revert: bool,

    #[arg(long, default_value_t = false)]
    ci: bool,

    #[arg(long)]
    confirm: bool,

    #[arg(short, long)]
    debug: bool,

    #[arg(long, default_value_t = false)]
    dry_run: bool,

    #[arg(short, long)]
    example: bool,

    #[arg(short, long, default_value_t = 2)]
    fuzziness: u8,

    #[arg(short = 'm', long, default_value_t = 0.7)]
    match_threshold: f32,

    #[arg(short, long)]
    verbose: bool,

    #[arg(
        short,
        long,
        default_value_t = false,
        conflicts_with_all = &["verbose", "debug", "confirm"]
    )]
    silent: bool,
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
    for (i, line) in source_lines
        .iter()
        .enumerate()
        .take(start_line)
        .skip(context_before_start)
    {
        eprintln!("  {:>4} | {}", i + 1, line);
    }
    eprintln!(
        "  ---- | --- (Patch would be applied here, replacing {} lines) ---",
        hunk_match.matched_length
    );
    let end_line = start_line + hunk_match.matched_length;
    for (i, line) in source_lines
        .iter()
        .enumerate()
        .take(min(source_lines.len(), end_line + CONTEXT_LINES))
        .skip(end_line)
    {
        eprintln!("  {:>4} | {}", i + 1, line);
    }
}

struct PatcherOptions {
    fuzziness: u8,
    debug_mode: bool,
    confirm: bool,
    ci: bool,
    silent: bool,
    match_threshold: f32,
}

fn resolve_file_diff_interactively(
    file_diff: &FileDiff,
    cli_target_path: &Option<String>,
    options: &PatcherOptions,
    report: &mut Report,
) -> Result<Option<FilePatchResult>, PatchError> {
    let old_path = cli_target_path
        .clone()
        .unwrap_or_else(|| file_diff.old_file.clone());
    let new_path = cli_target_path
        .clone()
        .unwrap_or_else(|| file_diff.new_file.clone());
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
            return Err(PatchError::IOError(format!(
                "Original file not found: {}",
                path.display()
            )));
        }
        if is_binary(path).unwrap_or(false) {
            report
                .warnings
                .push(format!("Skipped binary file: {old_path}"));
            return Ok(None);
        }
        fs::read_to_string(path)?
            .lines()
            .map(String::from)
            .collect()
    };

    let clean_source_map: Vec<(usize, String)> = source_lines
        .iter()
        .enumerate()
        .map(|(i, s)| (i, patcher::normalize_line(s)))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    let mut clean_index_map: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, norm) in &clean_source_map {
        clean_index_map.entry(norm.clone()).or_default().push(*idx);
    }
    for (i, hunk) in file_diff.hunks.iter().enumerate().rev() {
        loop {
            let possible_matches = patcher::find_hunk_location(
                &source_lines,
                &clean_source_map,
                &clean_index_map,
                hunk,
                options.fuzziness,
                options.debug_mode,
                options.match_threshold,
            );
            if possible_matches.is_empty() {
                if options.ci || options.silent {
                    return Err(PatchError::HunkApplicationFailed {
                        file_path: new_path.clone(),
                        hunk_index: i,
                        reason: "No matching context found in CI mode.".to_string(),
                    });
                }
                eprintln!(
                    "[ERROR] Failed to apply hunk {} for file {}. No matching context found.",
                    i + 1,
                    new_path
                );
                eprintln!("Do you want to [s]kip this hunk or [a]bort the process? (s/a)");
                let choice = read_user_input();
                if choice.to_lowercase() == "s" {
                    report.hunks_skipped += 1;
                    break;
                } else if choice.to_lowercase() == "a" {
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
                if options.ci || options.silent {
                    return Err(PatchError::AmbiguousMatch {
                        file_path: new_path.clone(),
                        hunk_index: i,
                    });
                }
                eprintln!(
                    "[ERROR] Ambiguous match for hunk {} in file {}. Possible locations:",
                    i + 1,
                    new_path
                );
                for (idx, m) in possible_matches.iter().enumerate() {
                    print_match_context(&source_lines, m, idx + 1);
                }
                eprintln!(
                    "\nEnter the index of the correct location, [s]kip this hunk, or [a]bort: "
                );
                let choice = read_user_input();
                if choice.to_lowercase() == "s" {
                    report.hunks_skipped += 1;
                    break;
                } else if choice.to_lowercase() == "a" {
                    return Err(PatchError::AmbiguousMatch {
                        file_path: new_path.clone(),
                        hunk_index: i,
                    });
                } else if let Ok(index) = choice.parse::<usize>() {
                    if index > 0 && index <= possible_matches.len() {
                        let chosen_match = &possible_matches[index - 1];
                        if chosen_match.score < 0.9 {
                            report.warnings.push(format!(
                                "Hunk {} in '{}' was applied with a fuzzy match score ({:.2}). Please review.",
                                i + 1,
                                new_path,
                                chosen_match.score
                            ));
                        }

                        report.hunks_applied += 1;
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
                if !options.ci && !options.silent && (options.confirm || chosen_match.score < 1.0) {
                    eprintln!(
                        "[INFO] Found a single match for hunk {} in file {}.",
                        i + 1,
                        new_path
                    );
                    print_match_context(&source_lines, chosen_match, 1);
                    eprintln!("\nApply this hunk? [y]es, [s]kip, [a]bort (y/s/a)");
                    let choice = read_user_input();
                    if choice.to_lowercase() == "y" {
                        if chosen_match.score < 0.9 {
                            report.warnings.push(format!(
                                "Hunk {} in '{}' was applied with a fuzzy match score ({:.2}). Please review.",
                                i + 1,
                                new_path,
                                chosen_match.score
                            ));
                        }

                        report.hunks_applied += 1;
                        source_lines = patcher::apply_hunk(
                            &source_lines,
                            hunk,
                            chosen_match.start_index,
                            chosen_match.matched_length,
                        );
                        break;
                    } else if choice.to_lowercase() == "s" {
                        report.hunks_skipped += 1;
                        break;
                    } else if choice.to_lowercase() == "a" {
                        return Err(PatchError::HunkApplicationFailed {
                            file_path: new_path.clone(),
                            hunk_index: i,
                            reason: "User aborted during confirmation.".to_string(),
                        });
                    } else {
                        eprintln!("Invalid choice. Please enter 'y', 's', or 'a'.");
                        continue;
                    }
                } else {
                    if chosen_match.score < 0.9 {
                        report.warnings.push(format!(
                            "Hunk {} in '{}' was applied with a fuzzy match score ({:.2}). Please review.",
                            i + 1,
                            new_path,
                            chosen_match.score
                        ));
                    }

                    report.hunks_applied += 1;
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
    }
    let new_content = source_lines.join("\n");
    if old_path == "/dev/null" {
        Ok(Some(FilePatchResult::Created {
            path: new_path,
            new_content,
        }))
    } else {
        Ok(Some(FilePatchResult::Modified {
            path: new_path,
            new_content,
        }))
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

fn get_diff_content(args: &Args) -> Result<String, AppError> {
    let is_verbose = args.verbose || args.debug;
    let diff_content = if args.clipboard {
        if is_verbose {
            println!("[INFO] Reading diff from clipboard...");
        }
        let mut ctx: ClipboardContext =
            ClipboardProvider::new().map_err(|e| AppError::Clipboard(e.to_string()))?;
        ctx.get_contents()
            .map_err(|e| AppError::Clipboard(e.to_string()))?
    } else {
        match &args.diff_file {
            Some(path) => {
                if is_verbose {
                    println!("[INFO] Reading diff from file: {path}");
                }
                fs::read_to_string(path)?
            }
            None => {
                if io::stdin().is_terminal() {
                    return Err(AppError::NoInput);
                }
                if is_verbose {
                    println!("[INFO] Reading diff from stdin...");
                }
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)?;
                buffer
            }
        }
    };
    Ok(diff_content)
}

fn process_patch(
    patch: &Patch,
    args: &Args,
    report: &mut Report,
) -> Result<Vec<FilePatchResult>, AppError> {
    let options = PatcherOptions {
        fuzziness: args.fuzziness,
        debug_mode: args.debug,
        confirm: args.confirm,
        ci: args.ci,
        silent: args.silent,
        match_threshold: args.match_threshold,
    };

    let mut all_patch_results: Vec<FilePatchResult> = Vec::new();
    let is_verbose = args.verbose || args.debug;

    for (i, file_diff) in patch.diffs.iter().enumerate() {
        if is_verbose {
            println!(
                "[INFO] Processing file diff {}/{} for file '{}'",
                i + 1,
                patch.diffs.len(),
                args.target_file.as_deref().unwrap_or(&file_diff.new_file)
            );
        }
        if let Some(result) =
            resolve_file_diff_interactively(file_diff, &args.target_file, &options, report)?
        {
            all_patch_results.push(result);
        }
    }
    Ok(all_patch_results)
}

fn handle_results(
    results: &[FilePatchResult],
    dry_run: bool,
    silent: bool,
    revert: bool,
    report: &mut Report,
    start_instant: Instant,
) -> io::Result<()> {
    for result in results {
        match result {
            FilePatchResult::Modified { .. } => report.files_modified += 1,
            FilePatchResult::Created { .. } => report.files_created += 1,
            FilePatchResult::Deleted { .. } => report.files_deleted += 1,
        }
    }

    if dry_run && !silent {
        println!("\n[DRY RUN] The following changes would be applied:");
        for result in results {
            match result {
                FilePatchResult::Modified { path, .. } => println!("  - [MODIFIED] {path}"),
                FilePatchResult::Created { path, .. } => println!("  - [CREATED]  {path}"),
                FilePatchResult::Deleted { path } => println!("  - [DELETED]  {path}"),
            }
        }
    }

    if !results.is_empty() {
        if !dry_run {
            apply_changes(results)?;
        }
        report.elapsed_ms = Some(start_instant.elapsed().as_millis());
        if !silent {
            println!("{}", report.summary(dry_run, revert));
        }
    } else if !silent {
        println!("No changes were applied.");
    }
    Ok(())
}

fn main_logic(mut args: Args) -> Result<(), AppError> {
    let is_verbose = (args.verbose || args.debug) && !args.silent;

    if !args.clipboard && args.target_file.is_some() && args.diff_file.is_none() {
        args.diff_file = args.target_file.take();
    }

    if is_verbose {
        println!("mend: A fuzzy diff applicator");
        println!("----------------------------");
    }

    let diff_content = get_diff_content(&args)?;

    if diff_content.is_empty() {
        return Err(AppError::EmptyDiff);
    }

    let mut patch = parser::parse_patch(&diff_content)?;

    if args.revert {
        if is_verbose {
            println!("[INFO] Inverting patch for revert operation...");
        }
        patch = patch.invert();
    }

    if let Some(target_path_str) = &args.target_file {
        let target_path = PathBuf::from(target_path_str);
        let target_filename = target_path.file_name().unwrap_or_default();

        patch.diffs.retain(|diff| {
            let old_filename = Path::new(&diff.old_file).file_name().unwrap_or_default();
            let new_filename = Path::new(&diff.new_file).file_name().unwrap_or_default();
            old_filename == target_filename || new_filename == target_filename
        });

        if patch.diffs.is_empty() {
            return Err(AppError::NoMatchingChanges {
                target_file: target_path_str.clone(),
            });
        }
    }

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

    let mut report = Report::default();
    let overall_start = Instant::now();
    let all_patch_results = process_patch(&patch, &args, &mut report)?;

    handle_results(
        &all_patch_results,
        args.dry_run || args.debug,
        args.silent,
        args.revert,
        &mut report,
        overall_start,
    )?;

    if is_verbose {
        println!("----------------------------");
        println!("Execution finished.");
    }
    Ok(())
}

fn run() -> Result<(), AppError> {
    let args = Args::parse();
    if args.example {
        println!("This is an example diff, please follow the same format.\n");
        println!("{EXAMPLE_DIFF}");
        return Ok(());
    }
    main_logic(args)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[ERROR] {e}");
        process::exit(1);
    }
}
