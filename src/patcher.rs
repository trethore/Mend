use crate::diff::{Hunk, Line};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum FilePatchResult {
    Modified {
        path: String,
        new_content: String,
    },
    Created {
        path: String,
        new_content: String,
    },
    Deleted {
        path: String,
    },
}

#[derive(Debug, Clone)]
pub struct HunkMatch {
    pub start_index: usize,
    pub matched_length: usize,
    pub score: f32,
}

#[derive(Debug)]
pub enum PatchError {
    HunkApplicationFailed {
        file_path: String,
        hunk_index: usize,
        reason: String,
    },
    AmbiguousMatch {
        file_path: String,
        hunk_index: usize,
    },
    IOError(String),
}

impl From<std::io::Error> for PatchError {
    fn from(err: std::io::Error) -> Self {
        PatchError::IOError(err.to_string())
    }
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::HunkApplicationFailed { file_path, hunk_index, reason } => {
                write!(f, "Failed to apply hunk {} for file {}: {}", hunk_index + 1, file_path, reason)
            }
            PatchError::AmbiguousMatch { file_path, hunk_index, .. } => {
                write!(f, "Ambiguous match for hunk {} in file {}", hunk_index + 1, file_path)
            }
            PatchError::IOError(e) => {
                write!(f, "I/O error: {}", e)
            }
        }
    }
}


fn deduplicate_matches(matches: Vec<HunkMatch>) -> Vec<HunkMatch> {
    if matches.len() <= 1 {
        return matches;
    }

    let mut best_matches: HashMap<usize, HunkMatch> = HashMap::new();

    for m in matches {
        let entry = best_matches.entry(m.start_index).or_insert_with(|| m.clone());

        if m.score > entry.score {
            *entry = m;
        }
        else if m.score == entry.score && m.matched_length > entry.matched_length {
            *entry = m;
        }
    }

    let best_score = best_matches.values().map(|m| m.score).fold(0.0, f32::max);

    let mut final_matches: Vec<HunkMatch> = best_matches
        .into_values()
        .filter(|m| m.score >= best_score * 0.9)
        .collect();

    final_matches.sort_by_key(|m| m.start_index);

    final_matches
}


pub fn find_hunk_location(
    source_lines: &[String],
    hunk: &Hunk,
    fuzziness: u8,
    debug_mode: bool,
    match_threshold: f32,
) -> Vec<HunkMatch> {
    let anchor_lines: Vec<&String> = hunk
        .lines
        .iter()
        .filter_map(|line| match line {
            Line::Context(text) | Line::Removal(text) => Some(text),
            Line::Addition(_) => None,
        })
        .collect();

    let mut matches = Vec::new();

    if anchor_lines.is_empty() {
        matches.push(HunkMatch {
            start_index: hunk.new_start.saturating_sub(1),
            matched_length: 0,
            score: 1.0,
        });
        return matches;
    }

    if debug_mode {
        println!("[DEBUG]   -> Trying strict match...");
    }
    if let Some(start_index) = source_lines
        .windows(anchor_lines.len())
        .position(|window| window.iter().zip(anchor_lines.iter()).all(|(s, a)| s == *a))
    {
        matches.push(HunkMatch {
            start_index,
            matched_length: anchor_lines.len(),
            score: 1.0,
        });
        return matches;
    }

    if fuzziness == 0 {
        return matches;
    }

    let clean_anchor: Vec<String> = anchor_lines
        .iter()
        .map(|s| normalize_line(s))
        .filter(|s| !s.is_empty())
        .collect();

    if clean_anchor.is_empty() {
        return matches;
    }

    if fuzziness >= 1 {
        if debug_mode {
            println!("[DEBUG]   -> Trying whitespace-insensitive match...");
        }
        for i in 0..=source_lines.len() {
            let mut consumed_lines = 0;
            let mut clean_source_window = Vec::new();
            for (line_offset, line) in source_lines.iter().skip(i).enumerate() {
                consumed_lines = line_offset + 1;
                let normalized = normalize_line(line);
                if !normalized.is_empty() {
                    clean_source_window.push(normalized);
                }
                if clean_source_window.len() >= clean_anchor.len() {
                    break;
                }
            }

            if clean_source_window == clean_anchor {
                matches.push(HunkMatch {
                    start_index: i,
                    matched_length: consumed_lines,
                    score: 0.9,
                });
            }
        }
        if !matches.is_empty() {
             return deduplicate_matches(matches);
        }
    }

    if fuzziness >= 2 {
        if debug_mode {
            println!("[DEBUG]   -> Trying anchor-point heuristic match...");
        }

        let top_anchor = clean_anchor.first();
        let bottom_anchor = clean_anchor.last();

        if top_anchor.is_none() || bottom_anchor.is_none() {
            return matches;
        }
        let top_anchor = top_anchor.unwrap();
        let bottom_anchor = bottom_anchor.unwrap();

        for (i, source_line) in source_lines.iter().enumerate() {
            if normalize_line(source_line) == *top_anchor {
                let search_window_end = (i + anchor_lines.len() + 50).min(source_lines.len());

                for (j, inner_source_line) in source_lines.iter().enumerate().skip(i + 1) {
                    if j >= search_window_end {
                        break;
                    }

                    if normalize_line(inner_source_line) == *bottom_anchor {
                        let start_index = i;
                        let length = j - i + 1;
                        let candidate_block = &source_lines[start_index..=j];

                        let score = calculate_match_score(&clean_anchor, candidate_block);
                        if debug_mode {
                            println!(
                                "[DEBUG]     - Candidate at lines {}-{} scored {:.2}",
                                i + 1,
                                j + 1,
                                score
                            );
                        }

                        if score >= match_threshold {
                            matches.push(HunkMatch {
                                start_index,
                                matched_length: length,
                                score,
                            });
                        }
                    }
                }
            }
        }
    }

    deduplicate_matches(matches)
}

fn calculate_match_score(clean_anchor: &[String], candidate_block: &[String]) -> f32 {
    let normalized_candidate_set: HashSet<String> = candidate_block
        .iter()
        .map(|s| normalize_line(s))
        .filter(|s| !s.is_empty())
        .collect();

    if normalized_candidate_set.is_empty() {
        return 0.0;
    }

    let mut matches = 0;
    for anchor_line in clean_anchor {
        if normalized_candidate_set.contains(anchor_line) {
            matches += 1;
        }
    }

    matches as f32 / clean_anchor.len() as f32
}

pub fn apply_hunk(
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