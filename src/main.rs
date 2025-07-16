use clap::Parser;
use std::fs;
use std::io::{self, Read, Write};

mod diff;
use diff::{Diff, Hunk, Line};

mod parser;
mod patcher;

#[derive(Parser, Debug)]
#[command(author = "Tytoo", version, about, long_about = None)]
struct Args {
    #[arg(index = 1)]
    original_file: String,
    #[arg(index = 2)]
    diff_file: Option<String>,
    #[arg(short, long)]
    in_place: bool,
    #[arg(long, default_value_t = true)]
    dry_run: bool,
    #[arg(short, long, default_value_t = 0)]
    fuzziness: u8,
}


fn main() -> io::Result<()> {
    let mut args = Args::parse();

    if !args.in_place {
        args.dry_run = true;
    }

    println!("mend: A fuzzy diff applicator");
    println!("----------------------------");

    let original_content = fs::read_to_string(&args.original_file)?;
    println!("[INFO] Read original file: {}", args.original_file);

    let diff_content = match &args.diff_file {
        Some(path) => {
            println!("[INFO] Read diff file: {}", path);
            fs::read_to_string(path)?
        }
        None => {
            println!("[INFO] Reading diff from stdin...");
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    let parsed_diff = parser::parse_diff(&diff_content);
    println!("[INFO] Parsed diff with {} hunk(s).", parsed_diff.hunks.len());

    println!("[INFO] Applying patches with fuzziness level {}.", args.fuzziness);
    match patcher::apply_diff(&original_content, &parsed_diff, args.fuzziness) {
        Ok(patched_content) => {
            println!("[SUCCESS] All hunks applied successfully.");

            if args.dry_run {
                println!("[INFO] Dry run. Patched content will be printed below.");
                println!("--- START OF PATCHED FILE ---");
                println!("{}", patched_content);
                println!("---  END OF PATCHED FILE  ---");
            }

            if args.in_place {
                println!("[INFO] Writing changes to {} in-place.", args.original_file);
                fs::write(&args.original_file, patched_content)?;
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