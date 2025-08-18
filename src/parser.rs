use crate::diff::{FileDiff, Hunk, Line, Patch};

pub fn parse_patch(patch_content: &str) -> Result<Patch, String> {
    let mut patch = Patch::default();
    let mut current_file_diff: Option<FileDiff> = None;

    let mut save_current_diff = |diff: Option<FileDiff>| {
        if let Some(mut d) = diff {
            if !d.hunks.is_empty() {
                if !d.old_file.is_empty() && d.old_file != "/dev/null" && d.new_file.is_empty() {
                    d.new_file = "/dev/null".to_string();
                }
                patch.diffs.push(d);
            }
        }
    };

    for raw_line in patch_content.lines() {
        let line = raw_line;

        if line.starts_with("diff --git ") {
            save_current_diff(current_file_diff.take());
            current_file_diff = Some(FileDiff::default());
            continue;
        }

        if line.starts_with("---") {
            if line.trim().chars().all(|c| c == '-') {
                continue;
            }

            if current_file_diff.is_none() {
                current_file_diff = Some(FileDiff::default());
            }

            if let Some(diff) = current_file_diff.as_mut() {
                let path_part = line[3..].trim();
                let path_candidate = if path_part.contains(char::is_whitespace) {
                    path_part.split_whitespace().last().unwrap_or(path_part)
                } else {
                    path_part
                };
                let final_path = path_candidate.strip_prefix("a/").unwrap_or(path_candidate);
                if final_path == "/dev/null" || final_path == "dev/null" {
                    diff.old_file = "/dev/null".to_string();
                } else {
                    diff.old_file = final_path.to_string();
                }
            }
            continue;
        }

        if line.starts_with("+++") {
            if line.trim().chars().all(|c| c == '+') {
                continue;
            }

            if current_file_diff.is_none() {
                current_file_diff = Some(FileDiff::default());
            }

            if let Some(diff) = current_file_diff.as_mut() {
                let path_part = line[3..].trim();
                let path_candidate = if path_part.contains(char::is_whitespace) {
                    path_part.split_whitespace().last().unwrap_or(path_part)
                } else {
                    path_part
                };
                let final_path = path_candidate.strip_prefix("b/").unwrap_or(path_candidate);
                if final_path == "/dev/null" || final_path == "dev/null" {
                    diff.new_file = "/dev/null".to_string();
                } else {
                    diff.new_file = final_path.to_string();
                }

                if diff.old_file.is_empty() {
                    diff.old_file = "/dev/null".to_string();
                }
            }
            continue;
        }

        if line.starts_with("@@") {
            if let Some(diff) = current_file_diff.as_mut() {
                diff.hunks.push(Hunk::default());
            } else {
                let mut diff = FileDiff::default();
                diff.hunks.push(Hunk::default());
                current_file_diff = Some(diff);
            }
            continue;
        }

        if line.starts_with("index ")
            || line.starts_with("new file mode ")
            || line.starts_with("deleted file mode ")
            || line.starts_with("similarity index ")
            || line.starts_with("rename from ")
            || line.starts_with("rename to ")
            || line.starts_with("Binary files ")
            || line.starts_with("\\ No newline at end of file")
        {
            continue;
        }

        if let Some(diff) = current_file_diff.as_mut() {
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