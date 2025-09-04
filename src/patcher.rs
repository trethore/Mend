use crate::diff::{Hunk, Line};
use lcs::LcsTable;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::error::Error;
use std::time::Instant;

#[derive(Debug)]
pub enum FilePatchResult {
    Modified { path: String, new_content: String },
    Created { path: String, new_content: String },
    Deleted { path: String },
}

#[derive(Debug, Clone)]
pub struct HunkMatch {
    pub start_index: usize,
    pub matched_length: usize,
    pub score: f32,
    pub density: f32,
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
            PatchError::HunkApplicationFailed {
                file_path,
                hunk_index,
                reason,
            } => {
                write!(
                    f,
                    "Failed to apply hunk {} for file {}: {}",
                    hunk_index + 1,
                    file_path,
                    reason
                )
            }
            PatchError::AmbiguousMatch {
                file_path,
                hunk_index,
                ..
            } => {
                write!(
                    f,
                    "Ambiguous match for hunk {} in file {}",
                    hunk_index + 1,
                    file_path
                )
            }
            PatchError::IOError(e) => {
                write!(f, "I/O error: {e}")
            }
        }
    }
}

impl Error for PatchError {}

fn get_indentation(s: &str) -> &str {
    s.find(|c: char| !c.is_whitespace()).map_or(s, |i| &s[..i])
}

fn apply_proximity_bonus(matches: &mut [HunkMatch], old_start_line: usize, debug_mode: bool) {
    const MAX_DISTANCE_FOR_BONUS: usize = 50;
    const MAX_BONUS: f32 = 0.05;

    if old_start_line == 0 {
        return;
    }
    if debug_mode {
        println!(
            "[DEBUG]   -> Applying proximity bonus based on original start line: {old_start_line}"
        );
    }

    for m in matches.iter_mut() {
        let distance = (m.start_index as i64 - (old_start_line.saturating_sub(1)) as i64)
            .unsigned_abs() as usize;
        if distance <= MAX_DISTANCE_FOR_BONUS {
            let bonus = MAX_BONUS * (1.0 - distance as f32 / MAX_DISTANCE_FOR_BONUS as f32);
            let old_score = m.score;
            m.score = (m.score + bonus).min(1.0);
            if debug_mode && m.score > old_score {
                println!(
                    "[DEBUG]     - Bonus for match at line {}: score {:.2} -> {:.2} (distance: {})",
                    m.start_index + 1,
                    old_score,
                    m.score,
                    distance
                );
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
        let entry = best_matches
            .entry(m.start_index)
            .or_insert_with(|| m.clone());

        if m.score > entry.score || (m.score == entry.score && m.density > entry.density) {
            *entry = m;
        }
    }

    let mut unique_matches: Vec<HunkMatch> = best_matches.into_values().collect();

    unique_matches.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| b.density.partial_cmp(&a.density).unwrap_or(Ordering::Equal))
            .then_with(|| a.start_index.cmp(&b.start_index))
    });

    if unique_matches.is_empty() {
        return unique_matches;
    }
    let best_score = unique_matches[0].score;
    unique_matches.retain(|m| m.score >= best_score * 0.9);

    unique_matches
}

fn find_best_anchor_in_slice<'a>(slice: &[&'a String]) -> Option<&'a String> {
    slice
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .max_by_key(|l| l.trim().len())
}

pub fn find_hunk_location(
    source_lines: &[String],
    clean_source_map: &[(usize, String)],
    clean_index_map: &HashMap<String, Vec<usize>>,
    hunk: &Hunk,
    fuzziness: u8,
    debug_mode: bool,
    match_threshold: f32,
) -> Vec<HunkMatch> {
    let total_start = if debug_mode { Some(Instant::now()) } else { None };
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
            start_index: hunk.old_start,
            matched_length: 0,
            score: 1.0,
            density: 1.0,
        });
        return matches;
    }

    if debug_mode {
        println!("[DEBUG]   -> Trying strict match...");
    }
    let strict_start = if debug_mode { Some(Instant::now()) } else { None };
    if let Some(start_index) = source_lines
        .windows(anchor_lines.len())
        .position(|window| window.iter().zip(anchor_lines.iter()).all(|(s, a)| s == *a))
    {
        matches.push(HunkMatch {
            start_index,
            matched_length: anchor_lines.len(),
            score: 1.0,
            density: 1.0,
        });
        if let Some(s) = strict_start {
            if debug_mode {
                println!(
                    "[DEBUG]   -> Strict: 1 match in {}ms",
                    s.elapsed().as_millis()
                );
            }
        }
        return matches;
    }
    if let Some(s) = strict_start {
        if debug_mode {
            println!(
                "[DEBUG]   -> Strict: 0 matches in {}ms",
                s.elapsed().as_millis()
            );
        }
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
        let ws_start = if debug_mode { Some(Instant::now()) } else { None };

        let clean_source_lines: Vec<&String> = clean_source_map.iter().map(|(_, s)| s).collect();

        for (clean_start_idx, window) in clean_source_lines.windows(clean_anchor.len()).enumerate()
        {
            if window
                .iter()
                .zip(clean_anchor.iter())
                .all(|(&s1, s2)| s1 == s2)
            {
                let original_start_index = clean_source_map[clean_start_idx].0;

                let clean_end_idx = clean_start_idx + clean_anchor.len() - 1;
                let original_end_index = clean_source_map[clean_end_idx].0;

                let matched_length = original_end_index - original_start_index + 1;
                let density = if matched_length > 0 {
                    clean_anchor.len() as f32 / matched_length as f32
                } else {
                    1.0
                };

                matches.push(HunkMatch {
                    start_index: original_start_index,
                    matched_length,
                    score: 0.9,
                    density,
                });
            }
        }

        if !matches.is_empty() {
            apply_proximity_bonus(&mut matches, hunk.old_start, debug_mode);
            let deduped = deduplicate_matches(matches);
            if let Some(s) = ws_start {
                if debug_mode {
                    println!(
                        "[DEBUG]   -> Whitespace-insensitive: {} match(es) in {}ms",
                        deduped.len(),
                        s.elapsed().as_millis()
                    );
                }
            }
            return deduped;
        }
        if let Some(s) = ws_start {
            if debug_mode {
                println!(
                    "[DEBUG]   -> Whitespace-insensitive: 0 matches in {}ms",
                    s.elapsed().as_millis()
                );
            }
        }
    }

    if fuzziness >= 2 {
        if debug_mode {
            println!("[DEBUG]   -> Trying anchor-point heuristic match...");
        }
        let anchor_start = if debug_mode { Some(Instant::now()) } else { None };

        let num_additions = hunk
            .lines
            .iter()
            .filter(|l| matches!(l, Line::Addition(_)))
            .count();
        let num_removals = hunk
            .lines
            .iter()
            .filter(|l| matches!(l, Line::Removal(_)))
            .count();
        let change_magnitude = num_additions + num_removals;

        let adaptive_window = anchor_lines.len() + std::cmp::max(10, 4 * change_magnitude);
        let search_window_size = std::cmp::min(adaptive_window, 400);

        if debug_mode {
            println!("[DEBUG]     - Adaptive search window size: {search_window_size}");
        }

        let (top_anchor_original, bottom_anchor_original) = if anchor_lines.len() > 2 {
            let mid_point = anchor_lines.len() / 2;
            let top = find_best_anchor_in_slice(&anchor_lines[..mid_point]);
            let bottom = find_best_anchor_in_slice(&anchor_lines[mid_point..]);

            match (top, bottom) {
                (Some(t), Some(b)) => (t, b),
                _ => (
                    *anchor_lines.first().unwrap(),
                    *anchor_lines.last().unwrap(),
                ),
            }
        } else {
            (
                *anchor_lines.first().unwrap(),
                *anchor_lines.last().unwrap(),
            )
        };

        let top_anchor_indent = get_indentation(top_anchor_original);
        let top_anchor = normalize_line(top_anchor_original);
        let bottom_anchor = normalize_line(bottom_anchor_original);

        if let Some(top_positions) = clean_index_map.get(&top_anchor) {
            if let Some(bottom_positions) = clean_index_map.get(&bottom_anchor) {
                let mut candidates_considered: usize = 0;
                for &original_idx_top in top_positions {
                    let search_window_end =
                        (original_idx_top + search_window_size).min(source_lines.len());

                    for &original_idx_bottom in bottom_positions {
                        if original_idx_bottom <= original_idx_top {
                            continue;
                        }
                        if original_idx_bottom >= search_window_end {
                            break;
                        }

                        candidates_considered += 1;
                        let start_index = original_idx_top;
                        let length = original_idx_bottom - start_index + 1;

                        let max_density = if length > 0 {
                            clean_anchor.len() as f32 / length as f32
                        } else {
                            1.0
                        };
                        let upper_bound = (0.7 * 1.0) + (0.3 * max_density);
                        if upper_bound < match_threshold {
                            continue;
                        }

                        let candidate_block = &source_lines[start_index..=original_idx_bottom];
                        let lcs_score = calculate_match_score(&clean_anchor, candidate_block);
                        let density = max_density;

                        let mut score = (0.7 * lcs_score) + (0.3 * density);

                        let candidate_top_anchor_line = &source_lines[original_idx_top];
                        let candidate_indent = get_indentation(candidate_top_anchor_line);
                        if top_anchor_indent == candidate_indent {
                            let original_score = score;
                            score = (score + 0.05).min(1.0);
                            if debug_mode && score > original_score {
                                println!(
                                    "[DEBUG]     - Indentation bonus applied. Score: {original_score:.2} -> {score:.2}"
                                );
                            }
                        }

                        if debug_mode {
                            println!(
                                "[DEBUG]     - Candidate at lines {}-{} scored {:.2} (LCS: {:.2}, Density: {:.2})",
                                start_index + 1,
                                original_idx_bottom + 1,
                                score,
                                lcs_score,
                                density
                            );
                        }

                        if score >= match_threshold {
                            matches.push(HunkMatch {
                                start_index,
                                matched_length: length,
                                score,
                                density,
                            });
                        }
                    }
                }
                if let Some(s) = anchor_start {
                    if debug_mode {
                        println!(
                            "[DEBUG]   -> Anchor heuristic: {} candidate pairs, {} kept, in {}ms",
                            candidates_considered,
                            matches.len(),
                            s.elapsed().as_millis()
                        );
                    }
                }
            }
        }
    }

    apply_proximity_bonus(&mut matches, hunk.old_start, debug_mode);
    let deduped = deduplicate_matches(matches);
    if let Some(s) = total_start {
        if debug_mode {
            println!(
                "[DEBUG]   -> Total matching: {} final match(es) in {}ms",
                deduped.len(),
                s.elapsed().as_millis()
            );
        }
    }
    deduped
}

fn calculate_match_score(clean_anchor: &[String], candidate_block: &[String]) -> f32 {
    if clean_anchor.is_empty() {
        return 1.0;
    }

    let normalized_candidate: Vec<String> = candidate_block
        .iter()
        .map(|s| normalize_line(s))
        .filter(|s| !s.is_empty())
        .collect();

    if normalized_candidate.is_empty() {
        return 0.0;
    }

    let table = LcsTable::new(clean_anchor, &normalized_candidate);
    let lcs = table.longest_common_subsequence();
    let lcs_len = lcs.len();

    lcs_len as f32 / clean_anchor.len() as f32
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

pub fn normalize_line(line: &str) -> String {
    line.trim().to_string()
}
