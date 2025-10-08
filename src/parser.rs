use crate::diff::{FileDiff, Hunk, Line, Patch};
use regex::Regex;

#[derive(Debug)]
pub struct ParseError {
    pub line_number: usize,
    pub line_content: String,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "on line {}: {}\n   L content: '{}'",
            self.line_number, self.message, self.line_content
        )
    }
}

fn sanitize_diff(input: &str) -> String {
    let mut lines: Vec<&str> = input.lines().collect();

    let start_fence_patterns = ["```diff", "```patch", "```"];
    let mut start_idx = 0;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if start_fence_patterns.iter().any(|pattern| trimmed == *pattern) {
            start_idx = i + 1;
            break;
        }
    }

    let mut end_idx = lines.len();
    if start_idx > 0 {
        for i in (start_idx..lines.len()).rev() {
            if lines[i].trim() == "```" {
                end_idx = i;
                break;
            }
        }
    }

    if start_idx > 0 && end_idx > start_idx {
        lines = lines[start_idx..end_idx].to_vec();
    }

    let diff_indicators = ["---", "+++", "@@", "diff --git"];
    let mut result = Vec::new();
    let mut in_hunk = false;
    let mut found_any_diff_marker = false;

    for line in lines {
        let trimmed = line.trim();

        if diff_indicators.iter().any(|marker| trimmed.starts_with(marker)) {
            found_any_diff_marker = true;
            result.push(line.to_string());
            if trimmed.starts_with("@@") {
                in_hunk = true;
            }
            continue;
        }

        if in_hunk {
            if !line.is_empty() && !line.starts_with('+') && !line.starts_with('-') && !line.starts_with(' ') {
                if line.chars().next().map_or(false, |c| c.is_whitespace()) {
                    result.push(line.to_string());
                } else if trimmed.is_empty() {
                    result.push(String::new());
                } else {
                    result.push(format!(" {}", line));
                }
            } else {
                result.push(line.to_string());
            }
        } else if found_any_diff_marker {
            if line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')
                || trimmed.starts_with("index ")
                || trimmed.starts_with("new file mode")
                || trimmed.starts_with("deleted file mode")
                || trimmed.starts_with("Binary files")
                || trimmed.starts_with("similarity index")
                || trimmed.starts_with("rename from")
                || trimmed.starts_with("rename to")
                || trimmed.starts_with("\\") {
                result.push(line.to_string());
            }
        }
    }

    result.join("\n")
}

pub fn parse_patch(patch_content: &str) -> Result<Patch, ParseError> {
    let sanitized = sanitize_diff(patch_content);
    let hunk_header_re =
        Regex::new(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").expect("Invalid regex");
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

    for (line_number, raw_line) in sanitized.lines().enumerate() {
        let line = raw_line;

        if line.starts_with("diff --git ") {
            save_current_diff(current_file_diff.take());
            current_file_diff = Some(FileDiff::default());
            continue;
        }

        if let Some(stripped) = line.strip_prefix("---") {
            if stripped.trim().is_empty() {
                continue;
            }
            if current_file_diff.is_none() {
                current_file_diff = Some(FileDiff::default());
            }
            if let Some(diff) = current_file_diff.as_mut() {
                let path_part = stripped.trim();
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

        if let Some(stripped) = line.strip_prefix("+++") {
            if stripped.trim().is_empty() {
                continue;
            }
            if current_file_diff.is_none() {
                current_file_diff = Some(FileDiff::default());
            }
            if let Some(diff) = current_file_diff.as_mut() {
                let path_part = stripped.trim();
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
            let mut new_hunk = Hunk::default();
            if let Some(caps) = hunk_header_re.captures(line) {
                let parse_num = |group: usize, default: usize| -> Result<usize, ParseError> {
                    caps.get(group)
                        .map_or(Ok(default), |m| m.as_str().parse::<usize>())
                        .map_err(|e| ParseError {
                            line_number: line_number + 1,
                            line_content: line.to_string(),
                            message: format!("Invalid number in hunk header: {e}"),
                        })
                };

                new_hunk.old_start = parse_num(1, 0)?;
                new_hunk.old_lines = parse_num(2, 1)?;
                new_hunk.new_start = parse_num(3, 0)?;
                new_hunk.new_lines = parse_num(4, 1)?;
            } else {
                return Err(ParseError {
                    line_number: line_number + 1,
                    line_content: line.to_string(),
                    message: "Malformed hunk header".to_string(),
                });
            }

            if let Some(diff) = current_file_diff.as_mut() {
                diff.hunks.push(new_hunk);
            } else {
                let mut diff = FileDiff::default();
                diff.hunks.push(new_hunk);
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
