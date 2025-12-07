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
fn test_intra_line_whitespace_mismatch() {
    // 1. ARRANGE
    // Original has extra spaces inside the function call
    let original_lines = to_lines(r#"
void func( int a, int b ) {
    return a + b;
}
"#);
    
    // Patch expects standard spacing
    let diff_content = r#"
@@ -1,3 +1,3 @@
 void func(int a, int b) {
-    return a + b;
+    return a * b;
 }
"#;

    let patch = parse_patch(diff_content).unwrap();
    let hunk = &patch.diffs[0].hunks[0];

    let (clean_source_map, clean_index_map) = build_clean_maps(&original_lines);
    
    // 2. ACT
    // Try with fuzziness 1 (Whitespace Insensitive)
    let matches: Vec<HunkMatch> = patcher::find_hunk_location(
        &original_lines,
        &clean_source_map,
        &clean_index_map,
        hunk,
        1, // Fuzziness 1
        true, // Debug on to see output
        0.7,
    );

    // 3. ASSERT
    // Now expecting SUCCESS with robust whitespace normalization.
    if matches.is_empty() {
        panic!("FAILED: Intra-line whitespace mismatch still causes failure at Level 1.");
    } else {
        println!("SUCCESS: Found {} match(es)! Best score: {}", matches.len(), matches[0].score);
        assert!(matches[0].score >= 0.9);
    }
}
