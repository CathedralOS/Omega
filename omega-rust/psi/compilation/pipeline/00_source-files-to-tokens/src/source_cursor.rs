//! Keep absolute byte coordinates while allowing comment scans to skip whole runs.

use std::str::Chars;

#[derive(Clone)]
pub(crate) struct SourceCursor<'source> {
    source_length: usize,
    chars: Chars<'source>,
}

impl<'source> SourceCursor<'source> {
    pub(crate) fn new(source: &'source str) -> Self {
        Self {
            source_length: source.len(),
            chars: source.chars(),
        }
    }

    pub(crate) fn offset(&self) -> usize {
        self.source_length - self.chars.as_str().len()
    }

    pub(crate) fn peek(&self) -> Option<(usize, char)> {
        self.chars
            .clone()
            .next()
            .map(|character| (self.offset(), character))
    }

    // Callers search ASCII delimiters, whose byte positions are UTF-8 boundaries.
    pub(crate) fn skip_until(&mut self, byte: u8) {
        debug_assert!(byte.is_ascii());
        let remaining = self.chars.as_str();
        let offset = memchr::memchr(byte, remaining.as_bytes()).unwrap_or(remaining.len());
        self.chars = remaining[offset..].chars();
    }

    pub(crate) fn skip_until_either(&mut self, first: u8, second: u8) {
        debug_assert!(first.is_ascii() && second.is_ascii());
        let remaining = self.chars.as_str();
        let offset =
            memchr::memchr2(first, second, remaining.as_bytes()).unwrap_or(remaining.len());
        self.chars = remaining[offset..].chars();
    }
}

impl Iterator for SourceCursor<'_> {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.offset();
        self.chars.next().map(|character| (offset, character))
    }
}
