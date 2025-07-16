use mend::{parser, patcher};
use std::fs;
use std::path::Path;

fn run_autodetect_test(diff_file_name: &str, expected_file_name: &str, expected_output_name: &str) {
    // 1. ARRANGE
    let diff_content = fs::read_to_string(Path::new("test/fixtures/diffs").join(diff_file_name))
        .expect("Failed to read diff file");

    // 2. ACT (Phase 1: Auto-detection)
    let detected_file = parser::find_target_file(&diff_content);

    // 3. ASSERT (Phase 1)
    assert!(detected_file.is_some(), "Should have detected a file path");

    let detected_path = Path::new(&detected_file.unwrap());
    assert_eq!(detected_path.file_name().unwrap(), Path::new(expected_file_name).file_name().unwrap());

    let original_content = fs::read_to_string(expected_file_name)
        .expect("Failed to read original file");
    let expected_content = fs::read_to_string(Path::new("test/fixtures/original").join(expected_output_name))
        .expect("Failed to read expected output file");

    // 2. ACT (Phase 2: Patching)
    let parsed_diff = parser::parse_diff(&diff_content);
    let result = patcher::apply_diff(&original_content, &parsed_diff, 2); // Use max fuzziness

    // 3. ASSERT (Phase 2)
    assert!(result.is_ok(), "Patching failed: {:?}", result.err());
    assert_eq!(result.unwrap().replace("\r\n", "\n"), expected_content.replace("\r\n", "\n"));
}

#[test]
fn test_autodetect_and_patch_greet() {
    let expected_output = r#"// A simple utility module.

pub fn greet(name: &str) -> String {
    // A more enthusiastic greeting.
    format!("Hello, {}! It is great to see you!", name)
}

pub fn farewell(name: &str) -> String {
    format!("Goodbye, {}.", name)
}
"#;
    fs::write("test/fixtures/original/expected_greet.rs", expected_output).unwrap();

    run_autodetect_test(
        "utils_greet.diff",
        "test/fixtures/original/utils.rs",
        "expected_greet.rs",
    );

    fs::remove_file("test/fixtures/original/expected_greet.rs").unwrap();
}

#[test]
fn test_autodetect_and_patch_farewell() {
    let expected_output = r#"// A simple utility module.

pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

pub fn farewell(name: &str) -> String {
    // A more formal farewell.
    format!("Goodbye and take care, {}.", name)
}
"#;
    fs::write("test/fixtures/original/expected_farewell.rs", expected_output).unwrap();

    run_autodetect_test(
        "utils_farewell.diff",
        "test/fixtures/original/utils.rs",
        "expected_farewell.rs",
    );

    fs::remove_file("test/fixtures/original/expected_farewell.rs").unwrap();
}