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
        println!("[INFO] Parsed patch with {} file diff(s).", patch.diffs.len());
        println!("[INFO] Applying patches with fuzziness level {}.", args.fuzziness);
    }

    match patcher::apply_patch(&patch, args.fuzziness, args.debug) {
        Ok(_) => {
            if is_verbose {
                println!("[SUCCESS] Patch applied successfully.");
            }
            println!("Successfully applied patch.");
        }
        Err(e) => {
            eprintln!("[ERROR] Could not apply patch: {}", e);
            std::process::exit(1);
        }
    }

    if is_verbose {
        println!("----------------------------");
        println!("Execution finished.");
    }

    Ok(())
}