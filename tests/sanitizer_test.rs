use mend::parser::parse_patch;

#[test]
fn test_sanitizer_strips_markdown_fences() {
    let diff_with_fences = r#"```diff
--- a/test.txt
+++ b/test.txt
@@ -1,1 +1,1 @@
-old line
+new line
```"#;

    let patch = parse_patch(diff_with_fences);
    assert!(patch.is_ok());
    let patch = patch.unwrap();
    assert_eq!(patch.diffs.len(), 1);
    assert_eq!(patch.diffs[0].hunks.len(), 1);
}

#[test]
fn test_sanitizer_removes_commentary() {
    let diff_with_commentary = r#"Here's the diff you requested:

```diff
--- a/test.txt
+++ b/test.txt
@@ -1,1 +1,1 @@
-old line
+new line
```

This changes old to new."#;

    let patch = parse_patch(diff_with_commentary);
    assert!(patch.is_ok());
    let patch = patch.unwrap();
    assert_eq!(patch.diffs.len(), 1);
}

#[test]
fn test_sanitizer_adds_missing_context_prefixes() {
    let diff_missing_prefixes = r#"--- a/test.txt
+++ b/test.txt
@@ -1,3 +1,3 @@
context line 1
-removed line
+added line
context line 2"#;

    let patch = parse_patch(diff_missing_prefixes);
    assert!(patch.is_ok());
    let patch = patch.unwrap();
    assert_eq!(patch.diffs.len(), 1);
    assert_eq!(patch.diffs[0].hunks.len(), 1);
}

#[test]
fn test_sanitizer_handles_multiple_fence_types() {
    let diff = r#"```patch
--- a/test.txt
+++ b/test.txt
@@ -1,1 +1,1 @@
-old
+new
```"#;

    let patch = parse_patch(diff);
    assert!(patch.is_ok());
}

#[test]
fn test_sanitizer_handles_plain_code_fence() {
    let diff = r#"```
--- a/test.txt
+++ b/test.txt
@@ -1,1 +1,1 @@
-old
+new
```"#;

    let patch = parse_patch(diff);
    assert!(patch.is_ok());
}
