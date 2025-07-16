use crate::diff::{Diff, Hunk, Line};
use once_cell::sync::Lazy;
use regex::Regex;

static HUNK_HEADER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@").unwrap());
.
pub fn parse_diff(diff_content: &str) -> Diff {
    let mut diff = Diff::default();

    for line in diff_content.lines() {
        if line.starts_with("@@") {
            let (original_start, new_start) = parse_hunk_header(line);
            let new_hunk = Hunk {
                original_start_line: original_start,
                new_start_line: new_start,
                ..Default::default()
            };
            diff.hunks.push(new_hunk);
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

fn parse_hunk_header(line: &str) -> (usize, usize) {
    HUNK_HEADER_RE
        .captures(line)
        .and_then(|caps| {
            let original = caps.get(1)?.as_str().parse::<usize>().ok();
            let new = caps.get(2)?.as_str().parse::<usize>().ok();
            Some((original?, new?))
        })
        .unwrap_or((1, 1))
}