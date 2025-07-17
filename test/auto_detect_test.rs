use mend::{parser, patcher};
use std::fs;
use std::path::Path;

fn run_autodetect_test(diff_file_name: &str, expected_target_filename: &str, expected_output_name: &str) {
    // SETUP: Ensure the test environment is ready.
    let work_dir = Path::new("test/work");
    if !work_dir.exists() {
        panic!("Test work directory 'test/work' does not exist. Please run test/setup_test_env.sh first.");
    }
    
    // 1. ARRANGE
    let diff_content = fs::read_to_string(Path::new("test/fixtures/diffs").join(diff_file_name))
        .expect("Failed to read diff file");
    
    let original_file_in_work_dir = work_dir.join(expected_target_filename);
    assert!(original_file_in_work_dir.exists(), "Original file {:?} not found in work directory. Did you run setup_test_env.sh?", original_file_in_work_dir);

    // 2. ACT (Phase 1: Auto-detection)
    let detected_file = parser::find_target_file(&diff_content);

    // 3. ASSERT (Phase 1)
    assert!(detected_file.is_some(), "Should have detected a file path");
    let detected_filename = detected_file.unwrap();
    assert_eq!(detected_filename, expected_target_filename, "Auto-detection failed to find the correct filename.");

    let original_content = fs::read_to_string(&original_file_in_work_dir)
        .expect("Failed to read original file from work directory");
    let expected_content = fs::read_to_string(Path::new("test/fixtures/original").join(expected_output_name))
        .expect("Failed to read expected output file");

    // 2. ACT (Phase 2: Patching)
    let parsed_diff = parser::parse_diff(&diff_content);
    let result = patcher::apply_diff(&original_content, &parsed_diff, 2, false); // Use max fuzziness

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
        "utils.rs",
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
        "utils.rs",
        "expected_farewell.rs",
    );

    fs::remove_file("test/fixtures/original/expected_farewell.rs").unwrap();
}