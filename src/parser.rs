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
                file_diff.hunks.push(Hunk::default());
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