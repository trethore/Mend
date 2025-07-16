#[derive(Debug, Clone)]
pub enum Line {
    Context(String),
    Addition(String),
    Removal(String),
}

#[derive(Debug, Default)]
pub struct Hunk {
    pub original_start_line: usize,
    pub new_start_line: usize,
    pub lines: Vec<Line>,
}

#[derive(Debug, Default)]
pub struct Diff {
    pub hunks: Vec<Hunk>,
}