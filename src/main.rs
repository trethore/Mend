use clap::Parser;
use is_terminal::IsTerminal;
use std::fs;
use std::io::{self, Read};

mod diff;
mod parser;
mod patcher;

#[derive(Parser, Debug)]
#[command(author = "Tytoo", version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    original_file: Option<String>,
    #[arg(index = 2)]
    diff_file: Option<String>,

    #[arg(long)]
    debug: bool,

    #[arg(short, long, default_value_t = 2)]
    fuzziness: u8,

    #[arg(short, long)]
    verbose: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let is_verbose = args.verbose || args.debug;

    if is_verbose {
        println!("mend: A fuzzy diff applicator");
        println!("----------------------------");
    }

    let (mut original_file_path, diff_file_path) =
        match (args.original_file, args.diff_file) {
            (Some(orig), Some(diff)) => (Some(orig), Some(diff)),
            (Some(file), None) => {
                if io::stdin().is_terminal() {
                    (None, Some(file))
                } else {
                    (Some(file), None)
                }
            }
            (None, None) => (None, None),
            (None, Some(_)) => unreachable!(),
        };

    let diff_content = match diff_file_path {
        Some(path) => {
            if is_verbose {
                println!("[INFO] Reading diff from file: {}", path);
            }
            fs::read_to_string(path)?
        }
        None => {
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

    if original_file_path.is_none() {
        if is_verbose {
            println!("[INFO] Original file not specified, searching in diff...");
        }
        match parser::find_target_file(&diff_content) {
            Some(path) => {
                if is_verbose {
                    println!("[INFO] Found target file in diff: {}", path);
                }
                original_file_path = Some(path);
            }
            None => {
                eprintln!("[ERROR] Could not determine original file from diff. Please specify it manually.");
                std::process::exit(1);
            }
        }
    }
    let original_file_path = original_file_path.unwrap();

    let original_content = fs::read_to_string(&original_file_path)?;
    if is_verbose {
        println!("[INFO] Read original file: {}", &original_file_path);
    }

    let parsed_diff = parser::parse_diff(&diff_content);
    if is_verbose {
        println!("[INFO] Parsed diff with {} hunk(s).", parsed_diff.hunks.len());
        println!("[INFO] Applying patches with fuzziness level {}.", args.fuzziness);
    }

    match patcher::apply_diff(&original_content, &parsed_diff, args.fuzziness, args.debug) {
        Ok(patched_content) => {
            if args.debug {
                println!("[SUCCESS] All hunks applied successfully (DEBUG MODE).");
                println!("--- START OF PATCHED FILE (not written to disk) ---");
                println!("{}", patched_content);
                println!("---  END OF PATCHED FILE (not written to disk)  ---");
            } else {
                if is_verbose {
                    println!("[SUCCESS] All hunks applied successfully.");
                    println!("[INFO] Writing changes to {} in-place.", original_file_path);
                }
                fs::write(&original_file_path, patched_content)?;
                println!("Successfully patched {}.", original_file_path);
            }
        }
        Err(e) => {
            eprintln!("[ERROR] Could not apply patch: {}", e);
            eprintln!("[FAIL] The file {} was NOT modified.", &original_file_path);
            std::process::exit(1);
        }
    }

    if is_verbose {
        println!("----------------------------");
        println!("Execution finished.");
    }

    Ok(())
}