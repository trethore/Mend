use crate::diff::{FileDiff, Hunk, Line, Patch};

pub fn parse_patch(patch_content: &str) -> Result<Patch, String> {
    let mut patch = Patch::default();
    let mut current_file_diff: Option<FileDiff> = None;

    let mut save_current_diff = |diff: Option<FileDiff>| {
        if let Some(d) = diff {
            if !d.hunks.is_empty() {
                patch.diffs.push(d);
            }
        }
    };

    for line in patch_content.lines() {
        if line.starts_with("--- ") {
            save_current_diff(current_file_diff.take());
            current_file_diff = Some(FileDiff::default());
            if let Some(diff) = current_file_diff.as_mut() {
                diff.old_file = line[4..].trim_start_matches("a/").to_string();
            }
            continue;
        }

        if line.starts_with("+++ ") {
            if current_file_diff.is_none() {
                current_file_diff = Some(FileDiff::default());
            }
            if let Some(diff) = current_file_diff.as_mut() {
                diff.new_file = line[4..].trim_start_matches("b/").to_string();
                if diff.old_file.is_empty() {
                    diff.old_file = diff.new_file.clone();
                }
            }
            continue;
        }

        if current_file_diff.is_none() {
            if line.starts_with("@@")
                || line.starts_with('+')
                || line.starts_with('-')
                || (line.starts_with(' ') && !line.trim().is_empty())
            {
                current_file_diff = Some(FileDiff::default());
            } else {
                continue;
            }
        }

        if let Some(diff) = current_file_diff.as_mut() {
            if line.starts_with("@@") {
                diff.hunks.push(Hunk::default());
                continue;
            }

            if diff.hunks.is_empty()
                && (line.starts_with('+') || line.starts_with('-') || line.starts_with(' '))
            {
                diff.hunks.push(Hunk::default());
            }

            if diff.hunks.is_empty() {
                continue;
            }

            if let Some(hunk) = diff.hunks.last_mut() {
                if let Some(text) = line.strip_prefix('+') {
                    hunk.lines.push(Line::Addition(text.to_string()));
                } else if let Some(text) = line.strip_prefix('-') {
                    hunk.lines.push(Line::Removal(text.to_string()));
                } else if let Some(text) = line.strip_prefix(' ') {
                    hunk.lines.push(Line::Context(text.to_string()));
                } else {
                    hunk.lines.push(Line::Context(line.to_string()));
                }
            }
        }
    }

    save_current_diff(current_file_diff.take());

    Ok(patch)
}