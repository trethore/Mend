#[derive(Debug, Clone)]
pub enum Line {
    Context(String),
    Addition(String),
    Removal(String),
}

#[derive(Debug, Default)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, Default)]
pub struct FileDiff {
    pub old_file: String,
    pub new_file: String,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Default)]
pub struct Patch {
    pub diffs: Vec<FileDiff>,
}