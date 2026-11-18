//! A byte range in one source file.

/// A half-open range of byte offsets: `start` is the first byte, `end` is one past the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn get_byte_count(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Borrows the bytes this span covers, or None if the span does not lie inside `source_bytes`.
    pub fn slice<'source>(&self, source_bytes: &'source [u8]) -> Option<&'source [u8]> {
        source_bytes.get(self.start..self.end)
    }
}
