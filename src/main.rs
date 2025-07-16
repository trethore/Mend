use clap::Parser;
use is_terminal::IsTerminal;
use std::fs;
use std::io::{self, Read};

mod diff;
use diff::{Diff, Hunk, Line};

mod parser;
mod patcher;

#[derive(Parser, Debug)]
#[command(author = "Tytoo", version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    original_file: Option<String>,
    #[arg(index = 2)]
    diff_file: Option<String>,
    #[arg(short, long)]
    in_place: bool,
    #[arg(short, long, default_value_t = 0)]
    fuzziness: u8,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let is_dry_run = !args.in_place;

    println!("mend: A fuzzy diff applicator");
    println!("----------------------------");

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
            println!("[INFO] Reading diff from file: {}", path);
            fs::read_to_string(path)?
        }
        None => {
            println!("[INFO] Reading diff from stdin...");
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
        println!("[INFO] Original file not specified, searching in diff...");
        match parser::find_target_file(&diff_content) {
            Some(path) => {
                println!("[INFO] Found target file in diff: {}", path);
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
    println!("[INFO] Read original file: {}", &original_file_path);

    let parsed_diff = parser::parse_diff(&diff_content);
    println!("[INFO] Parsed diff with {} hunk(s).", parsed_diff.hunks.len());

    println!("[INFO] Applying patches with fuzziness level {}.", args.fuzziness);
    match patcher::apply_diff(&original_content, &parsed_diff, args.fuzziness) {
        Ok(patched_content) => {
            println!("[SUCCESS] All hunks applied successfully.");

            if is_dry_run {
                println!("[INFO] Dry run. Patched content will be printed below.");
                println!("--- START OF PATCHED FILE ---");
                println!("{}", patched_content);
                println!("---  END OF PATCHED FILE  ---");
            }

            if args.in_place {
                println!("[INFO] Writing changes to {} in-place.", original_file_path);
                fs::write(&original_file_path, patched_content)?;
            }
        }
        Err(e) => {
            eprintln!("[ERROR] Could not apply patch: {}", e);
            std::process::exit(1);
        }
    }

    println!("----------------------------");
    println!("Execution finished.");

    Ok(())
}