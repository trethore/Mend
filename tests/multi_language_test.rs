use mend::parser::parse_patch;
use mend::patcher;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

type CleanSourceMap = Vec<(usize, String)>;
type CleanIndexMap = HashMap<String, Vec<usize>>;

fn to_lines(s: &str) -> Vec<String> {
    s.lines().map(String::from).collect()
}

fn build_clean_maps(lines: &[String]) -> (CleanSourceMap, CleanIndexMap) {
    let clean_source_map: CleanSourceMap = lines
        .iter()
        .enumerate()
        .map(|(i, s)| (i, patcher::normalize_line(s)))
        .filter(|(_, s)| !s.is_empty())
        .collect();

    let mut clean_index_map: CleanIndexMap = HashMap::new();
    for (idx, norm) in &clean_source_map {
        clean_index_map.entry(norm.clone()).or_default().push(*idx);
    }

    (clean_source_map, clean_index_map)
}

fn run_fixture_test(lang_dir: &str) {
    let base_path = Path::new("tests/fixtures/multilang").join(lang_dir);
    
    // Determine file extensions based on directory name
    let ext = match lang_dir {
        "python" => "py",
        "typescript" => "ts",
        "rust" => "rs",
        "lua" => "lua",
        _ => panic!("Unknown language fixture: {}", lang_dir),
    };

    let source_path = base_path.join(format!("source.{}", ext));
    let diff_path = base_path.join("patch.diff");
    let expected_path = base_path.join(format!("expected.{}", ext));

    let source_code = fs::read_to_string(&source_path).expect(&format!("Failed to read {:?}", source_path));
    let diff_content = fs::read_to_string(&diff_path).expect(&format!("Failed to read {:?}", diff_path));
    let expected_code = fs::read_to_string(&expected_path).expect(&format!("Failed to read {:?}", expected_path));

    println!("Testing language: {}", lang_dir);

    let original_lines = to_lines(&source_code);
    let patch = parse_patch(&diff_content).expect("Failed to parse diff");
    
    let mut current_lines = original_lines.clone();
    
    // Apply all hunks in the patch
    for file_diff in &patch.diffs {
        // In this test, we assume one file diff per fixture or we treat the single diff content as applying to the source.
        // Since our diffs might contain header lines or not, we just iterate hunks.
        
        let mut min_line = 0;
        
        for (hunk_idx, hunk) in file_diff.hunks.iter().enumerate() {
            // Build maps for fuzzy matching
            let (clean_source_map, clean_index_map) = build_clean_maps(&current_lines);
            
            let options = patcher::MatchOptions {
                fuzziness: 2,
                min_line,
                debug_mode: false,
                match_threshold: 0.5, // Generous threshold for tests
            };

            // Try strict first (mimic main loop logic briefly)
            let mut matches = patcher::find_strict_match(&current_lines, hunk, min_line, false);
            
            if matches.is_empty() {
                matches = patcher::find_fuzzy_match(
                    &current_lines,
                    &clean_source_map,
                    &clean_index_map,
                    hunk,
                    options,
                );
            }

            assert!(!matches.is_empty(), "Failed to match hunk {} for {}", hunk_idx + 1, lang_dir);
            
            let best_match = &matches[0];
            println!("  Hunk {} matched at line {} with score {:.2}", hunk_idx + 1, best_match.start_index + 1, best_match.score);
            
            current_lines = patcher::apply_hunk(
                &current_lines,
                hunk,
                best_match.start_index,
                best_match.matched_length
            );
            
             let hunk_new_lines_count = hunk.lines.iter().filter(|l| matches!(l, mend::diff::Line::Context(_) | mend::diff::Line::Addition(_))).count();
            min_line = best_match.start_index + hunk_new_lines_count;
        }
    }

    let result_code = current_lines.join("\n");
    
    // Normalize newlines for comparison
    let normalized_result = result_code.replace("\r\n", "\n").trim().to_string();
    let normalized_expected = expected_code.replace("\r\n", "\n").trim().to_string();

    if normalized_result != normalized_expected {
        println!("---
RESULT ---
{}
--- EXPECTED ---
{}", normalized_result, normalized_expected);
        panic!("Result does not match expected output for {}", lang_dir);
    }
}

#[test]
fn test_python_fixture() { run_fixture_test("python"); }

#[test]
fn test_typescript_fixture() { run_fixture_test("typescript"); }

#[test]
fn test_rust_fixture() { run_fixture_test("rust"); }

#[test]
fn test_lua_fixture() { run_fixture_test("lua"); }