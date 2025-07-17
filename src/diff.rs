#[derive(Debug, Clone)]
pub enum Line {
    Context(String),
    Addition(String),
    Removal(String),
}

#[derive(Debug, Default)]
pub struct Hunk {
    pub lines: Vec<Line>,
}

#[derive(Debug, Default)]
pub struct Diff {
    pub hunks: Vec<Hunk>,
}