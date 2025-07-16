use clap::Parser;
use std::fs;
use std::io::{self, Read};

mod diff;
use diff::{Diff, Hunk, Line};

mod parser;

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
    #[arg(short, long, default_value_t = 2)]
    fuzziness: u8,
}


fn main() -> io::Result<()> {
    let args = Args::parse();

    println!("mend: A fuzzy diff applicator");
    println!("----------------------------");

    let original_content = fs::read_to_string(&args.original_file)?;
    println!("[INFO] Read original file: {}", args.original_file);

    let diff_content = match args.diff_file {
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

    if let Some(first_hunk) = parsed_diff.hunks.first() {
        println!("[DEBUG] First hunk starts around original line {} and has {} lines of changes.",
            first_hunk.original_start_line,
            first_hunk.lines.len()
        );
    }


    println!("[TODO] Apply patches with fuzziness level {}.", args.fuzziness);


    if args.dry_run {
        println!("[INFO] Dry run enabled. Would write output to stdout.");
    } else if args.in_place {
        println!("[INFO] In-place enabled. Would modify {}.", args.original_file);
    } else {
        println!("[INFO] Would write output to a new file (not yet implemented).");
    }

    println!("----------------------------");
    println!("Execution finished (parser complete).");

    Ok(())
}