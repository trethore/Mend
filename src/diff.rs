// In mend/src/diff.rs

//! This module contains the data structures for representing a parsed diff.

/// Represents a single line within a hunk of a diff.
#[derive(Debug, Clone)]
pub enum Line {
    /// A line that exists in both files, used for context.
    /// Starts with a ' ' in the diff.
    Context(String),
    /// A line that is added in the new file.
    /// Starts with a '+' in the diff.
    Addition(String),
    /// A line that is removed from the old file.
    /// Starts with a '-' in the diff.
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