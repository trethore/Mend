use crate::diff::{FileDiff, Hunk, Line, Patch};

const DIFF_HEADER: &str = "diff --git ";

pub fn parse_patch(patch_content: &str) -> Result<Patch, String> {
    let mut patch = Patch::default();
    let mut current_file_diff: Option<FileDiff> = None;

    for line in patch_content.lines() {
        if line.starts_with(DIFF_HEADER) {
            if let Some(file_diff) = current_file_diff.take() {
                patch.diffs.push(file_diff);
            }
            current_file_diff = Some(FileDiff::default());
        } else if let Some(file_diff) = &mut current_file_diff {
            if line.starts_with("--- a/") {
                file_diff.old_file = line[6..].to_string();
            } else if line.starts_with("+++ b/") {
                file_diff.new_file = line[6..].to_string();
            } else if line.starts_with("@@") {
                if let Some(hunk_header) = line.strip_prefix("@@ ").and_then(|s| s.strip_suffix(" @@")) {
                    let parts: Vec<&str> = hunk_header.split(' ').collect();
                    if parts.len() == 2 {
                        let old_range: Vec<&str> = parts[0].strip_prefix("-").unwrap_or("").split(',').collect();
                        let new_range: Vec<&str> = parts[1].strip_prefix("+").unwrap_or("").split(',').collect();

                        let old_start = old_range[0].parse::<usize>().unwrap_or(0);
                        let old_lines = old_range.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
                        let new_start = new_range[0].parse::<usize>().unwrap_or(0);
                        let new_lines = new_range.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);

                        file_diff.hunks.push(Hunk {
                            _old_start: old_start,
                            _old_lines: old_lines,
                            new_start,
                            _new_lines: new_lines,
                            ..Default::default()
                        });
                    } else {
                        file_diff.hunks.push(Hunk::default());
                    }
                } else {
                    file_diff.hunks.push(Hunk::default());
                }
            } else if let Some(current_hunk) = file_diff.hunks.last_mut() {
                if let Some(text) = line.strip_prefix('+') {
                    current_hunk.lines.push(Line::Addition(text.to_string()));
                } else if let Some(text) = line.strip_prefix('-') {
                    current_hunk.lines.push(Line::Removal(text.to_string()));
                } else if let Some(text) = line.strip_prefix(' ') {
                    current_hunk.lines.push(Line::Context(text.to_string()));
                }
            }
        }
    }

    if let Some(file_diff) = current_file_diff.take() {
        patch.diffs.push(file_diff);
    }

    Ok(patch)
}