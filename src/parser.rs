use crate::diff::{Diff, Hunk, Line};
use once_cell::sync::Lazy;
use regex::Regex;

static FILE_PATH_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^--- (?:a/)?([^\t\n]+)").unwrap());

pub fn parse_diff(diff_content: &str) -> Diff {
    let mut diff = Diff::default();

    for line in diff_content.lines() {
        if line.starts_with("@@") {
            diff.hunks.push(Hunk::default());
        } else if let Some(current_hunk) = diff.hunks.last_mut() {
            if let Some(text) = line.strip_prefix('+') {
                current_hunk.lines.push(Line::Addition(text.to_string()));
            } else if let Some(text) = line.strip_prefix('-') {
                current_hunk.lines.push(Line::Removal(text.to_string()));
            } else if let Some(text) = line.strip_prefix(' ') {
                current_hunk.lines.push(Line::Context(text.to_string()));
            }
        }
    }

    diff
}

pub fn find_target_file(diff_content: &str) -> Option<String> {
    for line in diff_content.lines() {
        if line.starts_with("--- ") {
            if let Some(caps) = FILE_PATH_RE.captures(line) {
                if let Some(path) = caps.get(1) {
                    let path_str = path.as_str().trim();
                    if path_str != "/dev/null" { return Some(path_str.to_string()); }
                }
            }
        }
    }
    None
}