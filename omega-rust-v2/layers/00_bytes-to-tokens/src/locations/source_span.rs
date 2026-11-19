//! A byte range in one source file.

/// A half-open range of byte offsets: `start_offset` is the first byte, `end_offset` is one past the last.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    start_offset: usize,
    end_offset: usize,
}

impl SourceSpan {
    pub fn new(
        start_offset: usize,
        end_offset: usize,
    ) -> Self {
        Self { start_offset, end_offset }
    }

    pub fn get_start_offset(&self) -> usize {
        self.start_offset
    }

    pub fn get_end_offset(&self) -> usize {
        self.end_offset
    }

    pub fn get_byte_count(&self) -> usize {
        self.end_offset.saturating_sub(self.start_offset)
    }

    /// Borrows the bytes this span covers, or None if the span does not lie inside `source_bytes`.
    pub fn slice<'source>(
        &self,
        source_bytes: &'source [u8],
    ) -> Option<&'source [u8]> {
        source_bytes.get(self.start_offset..self.end_offset)
    }
}
