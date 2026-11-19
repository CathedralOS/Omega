//! Immutable byte views and coordinates over text, without loading or compiler state.
#![forbid(unsafe_code)]

use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, record_storage::Record)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, record_storage::Record)]
pub struct SourceId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, record_storage::Record)]
pub struct SourceSpan {
    pub source_id: SourceId,
    pub span: Span,
}

impl SourceSpan {
    pub fn new(source_id: SourceId, span: Span) -> Self {
        Self { source_id, span }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    pub fn of(source: &str, byte_offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;

        for (index, character) in source.char_indices() {
            if index >= byte_offset {
                break;
            }

            if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }

        SourcePosition { line, column }
    }
}

#[derive(Clone, record_storage::Record)]
pub struct SourceBytes {
    storage: Arc<Vec<u8>>,
    start: usize,
    end: usize,
}

impl SourceBytes {
    pub fn from_shared(storage: Arc<Vec<u8>>, range: std::ops::Range<usize>) -> Self {
        assert!(range.start <= range.end && range.end <= storage.len());
        Self {
            storage,
            start: range.start,
            end: range.end,
        }
    }
}

impl Default for SourceBytes {
    fn default() -> Self {
        Self::from(Vec::new())
    }
}

impl From<Vec<u8>> for SourceBytes {
    fn from(bytes: Vec<u8>) -> Self {
        let end = bytes.len();
        Self {
            storage: Arc::new(bytes),
            start: 0,
            end,
        }
    }
}

impl From<&[u8]> for SourceBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::from(bytes.to_vec())
    }
}

impl<const LENGTH: usize> From<&[u8; LENGTH]> for SourceBytes {
    fn from(bytes: &[u8; LENGTH]) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl Deref for SourceBytes {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &[u8] {
        &self.storage[self.start..self.end]
    }
}

impl AsRef<[u8]> for SourceBytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl fmt::Debug for SourceBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_ref(), formatter)
    }
}

impl PartialEq for SourceBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for SourceBytes {}

impl std::hash::Hash for SourceBytes {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}

pub fn display_literal_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(4).saturating_add(2));
    output.push('"');
    for byte in bytes {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'"' => output.push_str("\\\""),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(*byte)),
            _ => output.push_str(&format!("\\x{byte:02x}")),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_view_equality_uses_contents_independent_of_owner_and_offset() {
        let storage = Arc::new(vec![9, 0, 255, 8]);
        let view = SourceBytes::from_shared(storage.clone(), 1..3);
        let separate = SourceBytes::from(vec![0, 255]);
        assert_eq!(view, separate);
        assert_ne!(view, SourceBytes::from_shared(storage, 0..2));
        assert_eq!(format!("{view:?}"), "[0, 255]");
    }
}
