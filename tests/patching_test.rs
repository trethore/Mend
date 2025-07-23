use mend::{parser, patcher};

#[test]
fn test_strict_patch_succeeds() {
    // 1. ARRANGE: Set up a small, self-contained test case.
    let original = "line one\nline two\nline three";
    let diff = "@@ -1,3 +1,3 @@\n line one\n-line two\n+line two new\n line three";
    let expected = "line one\nline two new\nline three";

    // 2. ACT: Run the code you want to test.
    let parsed_diff = parser::parse_diff(diff);
    let result = patcher::apply_diff(original, &parsed_diff, 0, false);

    // 3. ASSERT: Check if the result is what you expected.
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
}

#[test]
fn test_fuzzy_patch_succeeds_when_strict_fails() {
    // ARRANGE: A case where Level 1 fuzziness is required.
    let original = "header\n\nline one\nline two\nline three";
    let diff = "@@ -1,3 +1,3 @@\n line one\n-line two\n+line two new\n line three";
    let expected = "header\n\nline one\nline two new\nline three";

    // ACT: Run with fuzziness 1.
    let parsed_diff = parser::parse_diff(diff);
    let result = patcher::apply_diff(original, &parsed_diff, 1, false);

    // ASSERT: Check the result.
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected);
}