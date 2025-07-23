use crate::diff::{FileDiff, Hunk, Line, Patch};
use std::collections::HashSet;
use std::fs;

const MATCH_SCORE_THRESHOLD: f32 = 0.7;

pub fn apply_patch(
    patch: &Patch,
    fuzziness: u8,
    debug_mode: bool,
) -> Result<(), String> {
    for (i, file_diff) in patch.diffs.iter().enumerate() {
        if debug_mode {
            println!(
                "[DEBUG] Applying diff {}/{} to file {}",
                i + 1,
                patch.diffs.len(),
                file_diff.new_file
            );
        }
        apply_file_diff(file_diff, fuzziness, debug_mode)?;
    }
    Ok(())
}

fn apply_file_diff(
    file_diff: &FileDiff,
    fuzziness: u8,
    debug_mode: bool,
) -> Result<(), String> {
    let original_content = match fs::read_to_string(&file_diff.old_file) {
        Ok(content) => content,
        Err(e) => return Err(format!("Failed to read file {}: {}", file_diff.old_file, e)),
    };

    let mut source_lines: Vec<String> = original_content.lines().map(String::from).collect();

    for (i, hunk) in file_diff.hunks.iter().enumerate() {
        match find_hunk_location(&source_lines, hunk, fuzziness, debug_mode) {
            Some((start_index, matched_length)) => {
                if debug_mode {
                    println!(
                        "[DEBUG] Hunk {}/{} matched at line {} (length {} lines)",
                        i + 1,
                        file_diff.hunks.len(),
                        start_index + 1,
                        matched_length
                    );
                }
                source_lines = apply_hunk(&source_lines, hunk, start_index, matched_length);
            }
            None => {
                return Err(format!(
                    "Failed to apply hunk {}/{}. Could not find matching context.",
                    i + 1,
                    file_diff.hunks.len()
                ));
            }
        }
    }

    let new_content = source_lines.join("\n");
    match fs::write(&file_diff.new_file, new_content) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to write file {}: {}", file_diff.new_file, e)),
    }
}

fn find_hunk_location(
    source_lines: &[String],
    hunk: &Hunk,
    fuzziness: u8,
    debug_mode: bool,
) -> Option<(usize, usize)> {
    let anchor_lines: Vec<&String> = hunk
        .lines
        .iter()
        .filter_map(|line| match line {
            Line::Context(text) | Line::Removal(text) => Some(text),
            Line::Addition(_) => None,
        })
        .collect();

    if anchor_lines.is_empty() { return None; }

    if debug_mode {
        println!("[DEBUG]   -> Trying strict match...");
    }
    if let Some(start_index) = source_lines.windows(anchor_lines.len()).position(|window| {
        window.iter().zip(anchor_lines.iter()).all(|(s, a)| s == *a)
    }) {
        return Some((start_index, anchor_lines.len()));
    }

    if fuzziness == 0 { return None; }

    let clean_anchor: Vec<String> = anchor_lines
        .iter()
        .map(|s| normalize_line(s))
        .filter(|s| !s.is_empty())
        .collect();

    if clean_anchor.is_empty() { return None; }

    if fuzziness >= 1 {
        if debug_mode {
            println!("[DEBUG]   -> Trying whitespace-insensitive match...");
        }
        for i in 0..source_lines.len() {
            let mut consumed_lines = 0;
            let mut clean_source_window = Vec::new();
            for (line_offset, line) in source_lines.iter().skip(i).enumerate() {
                consumed_lines = line_offset + 1;
                let normalized = normalize_line(line);
                if !normalized.is_empty() {
                    clean_source_window.push(normalized);
                }
                if clean_source_window.len() == clean_anchor.len() {
                    break;
                }
            }
            if clean_source_window == clean_anchor {
                return Some((i, consumed_lines));
            }
        }
    }

    if fuzziness >= 2 {
        if debug_mode {
            println!("[DEBUG]   -> Trying anchor-point heuristic match...");
        }

        let top_anchor = clean_anchor.first()?;
        let bottom_anchor = clean_anchor.last()?;

        let mut best_match: Option<(usize, usize, f32)> = None;

        for (i, source_line) in source_lines.iter().enumerate() {
            if normalize_line(source_line) == *top_anchor {
                let search_window_end = (i + anchor_lines.len() + 20).min(source_lines.len());

                for (j, inner_source_line) in source_lines.iter().enumerate().skip(i) {
                    if j >= search_window_end { break; }

                    if normalize_line(inner_source_line) == *bottom_anchor {
                        let start_index = i;
                        let length = j - i + 1;
                        let candidate_block = &source_lines[start_index..=j];

                        let score = calculate_match_score(&clean_anchor, candidate_block);
                        if debug_mode {
                            println!("[DEBUG]     - Candidate at lines {}-{} scored {:.2}", i + 1, j + 1, score);
                        }

                        if best_match.is_none() || score > best_match.as_ref().unwrap().2 {
                            best_match = Some((start_index, length, score));
                        }
                    }
                }
            }
        }

        if let Some((start, len, score)) = best_match {
            if score >= MATCH_SCORE_THRESHOLD {
                if debug_mode {
                    println!("[DEBUG]   -> Best anchor-point match found with score {:.2}. Accepting.", score);
                }
                return Some((start, len));
            }
        }
    }

    None
}

fn calculate_match_score(clean_anchor: &[String], candidate_block: &[String]) -> f32 {
    let normalized_candidate_set: HashSet<String> = candidate_block
        .iter()
        .map(|s| normalize_line(s))
        .filter(|s| !s.is_empty())
        .collect();

    if normalized_candidate_set.is_empty() { return 0.0; }

    let mut matches = 0;
    for anchor_line in clean_anchor {
        if normalized_candidate_set.contains(anchor_line) {
            matches += 1;
        }
    }

    matches as f32 / clean_anchor.len() as f32
}

fn apply_hunk(
    source_lines: &[String],
    hunk: &Hunk,
    start_index: usize,
    matched_length: usize,
) -> Vec<String> {
    let mut result = Vec::new();
    result.extend_from_slice(&source_lines[0..start_index]);
    for line in &hunk.lines {
        if let Line::Context(text) | Line::Addition(text) = line {
            result.push(text.clone());
        }
    }
    let end_of_patch_index = start_index + matched_length;
    if end_of_patch_index < source_lines.len() {
        result.extend_from_slice(&source_lines[end_of_patch_index..]);
    }
    result
}

fn normalize_line(line: &str) -> String {
    line.trim().to_string()
}