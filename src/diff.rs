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

impl Hunk {
    pub fn invert(&self) -> Hunk {
        let inverted_lines = self
            .lines
            .iter()
            .map(|line| match line {
                Line::Context(s) => Line::Context(s.clone()),
                Line::Addition(s) => Line::Removal(s.clone()),
                Line::Removal(s) => Line::Addition(s.clone()),
            })
            .collect();

        Hunk {
            old_start: self.new_start,
            old_lines: self.new_lines,
            new_start: self.old_start,
            new_lines: self.old_lines,
            lines: inverted_lines,
        }
    }
}

#[derive(Debug, Default)]
pub struct FileDiff {
    pub old_file: String,
    pub new_file: String,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn invert(&self) -> FileDiff {
        FileDiff {
            old_file: self.new_file.clone(),
            new_file: self.old_file.clone(),
            hunks: self.hunks.iter().map(|h| h.invert()).collect(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Patch {
    pub diffs: Vec<FileDiff>,
}

impl Patch {
    pub fn invert(&self) -> Patch {
        Patch {
            diffs: self.diffs.iter().map(|d| d.invert()).collect(),
        }
    }
}