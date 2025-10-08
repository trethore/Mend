use mend::diff::{Hunk, Line};
use mend::parser::parse_patch;
use mend::patcher::{self, HunkMatch};
use std::collections::HashMap;

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

#[test]
fn test_strict_patch_succeeds() {
    // 1. ARRANGE: Set up a small, self-contained test case.
    let original_lines = to_lines("line one\nline two\nline three");
    let diff_content = "@@ -1,3 +1,3 @@\n line one\n-line two\n+line two new\n line three";
    let expected = "line one\nline two new\nline three";

    // 2. ACT: Run the code you want to test.
    let patch = parse_patch(diff_content).unwrap();
    let hunk = &patch.diffs[0].hunks[0];

    let (clean_source_map, clean_index_map) = build_clean_maps(&original_lines);
    let matches: Vec<HunkMatch> = patcher::find_hunk_location(
        &original_lines,
        &clean_source_map,
        &clean_index_map,
        hunk,
        0,
        false,
        0.7,
    );

    // 3. ASSERT: Check that we found exactly one, perfect match.
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].score, 1.0);
    assert_eq!(matches[0].start_index, 0);

    // 2. ACT (Part 2): Apply the hunk
    let result_lines = patcher::apply_hunk(
        &original_lines,
        hunk,
        matches[0].start_index,
        matches[0].matched_length,
    );
    let result_str = result_lines.join("\n");

    // 3. ASSERT (Part 2): Check if the result is what you expected.
    assert_eq!(result_str, expected);
}

#[test]
fn test_fuzzy_patch_succeeds_when_strict_fails() {
    // ARRANGE: A case where Level 1 fuzziness (whitespace-insensitive) is required.
    let original_lines = to_lines("header\n\nline one\n    line two\nline three");
    let diff_content = "@@ -1,3 +1,3 @@\n line one\n-line two\n+line two new\n line three";
    let expected = "header\n\nline one\nline two new\nline three";

    // ACT: Run with fuzziness level 1.
    let patch = parse_patch(diff_content).unwrap();
    let hunk = &patch.diffs[0].hunks[0];

    let (clean_source_map, clean_index_map) = build_clean_maps(&original_lines);
    let matches: Vec<HunkMatch> = patcher::find_hunk_location(
        &original_lines,
        &clean_source_map,
        &clean_index_map,
        hunk,
        1,
        false,
        0.7,
    );

    // ASSERT: Check that we found a good match.
    assert_eq!(matches.len(), 1);
    assert!((matches[0].score - 0.9).abs() < 0.1);
    assert_eq!(matches[0].start_index, 2); // Should match at "line one"

    // ACT (Part 2): Apply the hunk
    let result_lines = patcher::apply_hunk(
        &original_lines,
        hunk,
        matches[0].start_index,
        matches[0].matched_length,
    );
    let result_str = result_lines.join("\n");

    // ASSERT (Part 2): Check the result.
    assert_eq!(result_str, expected);
}

#[test]
fn test_anchor_point_heuristic_succeeds() {
    // ARRANGE: A case where context lines have changed, requiring Level 2 fuzziness.
    let original_lines = to_lines("line one\nSOMETHING UNEXPECTED\nline three");
    let hunk = Hunk {
        lines: vec![
            Line::Context("line one".to_string()),
            Line::Removal("line two".to_string()),
            Line::Addition("line two new".to_string()),
            Line::Context("line three".to_string()),
        ],
        ..Default::default()
    };
    let expected = "line one\nline two new\nline three";

    let (clean_source_map, clean_index_map) = build_clean_maps(&original_lines);
    let matches: Vec<HunkMatch> = patcher::find_hunk_location(
        &original_lines,
        &clean_source_map,
        &clean_index_map,
        &hunk,
        2,
        false,
        0.7,
    );

    // ASSERT: Check that we found a match using the heuristic.
    assert_eq!(matches.len(), 1);
    assert!(matches[0].score >= 0.7); // Level 2 score
    assert_eq!(matches[0].start_index, 0);

    // ACT (Part 2): Apply the hunk
    let result_lines = patcher::apply_hunk(
        &original_lines,
        &hunk,
        matches[0].start_index,
        matches[0].matched_length,
    );
    let result_str = result_lines.join("\n");

    // ASSERT (Part 2): Check the result.
    assert_eq!(result_str, expected);
}
