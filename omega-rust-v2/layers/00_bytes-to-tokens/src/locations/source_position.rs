//! The line and column a person would look for, given a byte offset.

/// A one-based line and column. The column counts bytes, not characters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    line: usize,
    column: usize,
}

impl SourcePosition {
    pub fn new(
        line: usize,
        column: usize,
    ) -> Self {
        Self { line, column }
    }

    pub fn get_line(&self) -> usize {
        self.line
    }

    pub fn get_column(&self) -> usize {
        self.column
    }
}
