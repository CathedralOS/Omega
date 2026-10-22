//! An immutable view into one decoded source buffer.
//!
//! Lexing appends bytes to a source-local buffer. Retaining a literal shares
//! that buffer and its range instead of allocating and copying every payload.
//! Clones preserve independent ownership across syntax and semantic phases.

use std::{fmt, ops::Deref, sync::Arc};

#[derive(Clone)]
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

#[cfg(test)]
mod tests {
    use super::SourceBytes;
    use std::sync::Arc;

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
