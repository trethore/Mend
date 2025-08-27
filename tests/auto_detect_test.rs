use mend::parser;
use std::fs;
use std::path::Path;

fn run_parser_test(diff_file_name: &str, expected_old_file: &str, expected_new_file: &str) {
    // 1. ARRANGE
    let diff_content = fs::read_to_string(Path::new("test/fixtures/diffs").join(diff_file_name))
        .expect("Failed to read diff file");

    // 2. ACT
    let patch = parser::parse_patch(&diff_content).expect("Parsing the patch should succeed.");

    // 3. ASSERT
    assert_eq!(
        patch.diffs.len(),
        1,
        "Should have parsed exactly one file diff."
    );
    let file_diff = &patch.diffs[0];

    assert_eq!(
        file_diff.old_file, expected_old_file,
        "Detected old file path does not match."
    );
    assert_eq!(
        file_diff.new_file, expected_new_file,
        "Detected new file path does not match."
    );
    assert!(
        !file_diff.hunks.is_empty(),
        "Parsed diff should contain hunks."
    );
}

#[test]
fn test_parses_git_style_paths() {
    run_parser_test("utils_greet.diff", "utils.rs", "utils.rs");
}

#[test]
fn test_parses_custom_style_paths() {
    run_parser_test("claude.diff", "Personne.java.old", "Personne.java.new");
}

#[test]
fn test_parses_file_creation_paths() {
    // ARRANGE
    let diff_content = r#"
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,2 @@
+Hello
+World
"#;
    // ACT
    let patch = parser::parse_patch(diff_content).unwrap();

    // ASSERT
    assert_eq!(patch.diffs.len(), 1);
    let file_diff = &patch.diffs[0];
    assert_eq!(file_diff.old_file, "/dev/null");
    assert_eq!(file_diff.new_file, "new_file.txt");
}

#[test]
fn test_parses_diff_with_no_headers() {
    // ARRANGE: A diff with no --- or +++ lines.
    let diff_content = r#"
@@ -1,3 +1,3 @@
 line one
-line two
+line two new
 line three
"#;
    // ACT
    let patch = parser::parse_patch(diff_content).unwrap();

    // ASSERT
    assert_eq!(patch.diffs.len(), 1);
    let file_diff = &patch.diffs[0];
    assert!(file_diff.old_file.is_empty());
    assert!(file_diff.new_file.is_empty());
    assert_eq!(file_diff.hunks.len(), 1);
}
