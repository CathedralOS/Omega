//! Invocation-private byte backing with immutable shared views.
//! Storage identity never enters Terminal bytes.
//!
//! `binding.rs` distinguishes a frame's immutable byte values from its field
//! loans and prepares structural call arguments over them; `subslice.rs`
//! replaces one operation-owned immutable descriptor; `write.rs` mutates a
//! fixed extent through an existing mutable field loan; `field_store.rs`
//! replaces a bounded byte field's whole contents; `field_index_store.rs`
//! observes a field's length and stores one byte through it.

pub(crate) mod binding;
pub(crate) mod field_index_store;
pub(crate) mod field_store;
pub(crate) mod subslice;
pub(crate) mod write;

use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ByteSequenceView {
    backing: Arc<[u8]>,
    window: Range<usize>,
}

impl ByteSequenceView {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        let length = bytes.len();
        Self {
            backing: bytes.into(),
            window: 0..length,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.backing[self.window.clone()]
    }

    pub(crate) fn get(&self, index: usize) -> Option<&u8> {
        self.bytes().get(index)
    }

    /// Replace one live byte without changing another view's immutable value.
    /// Shared backing detaches only this window, not unused source bytes.
    pub(crate) fn replace_byte(&mut self, byte_index: usize, value: u8) -> Option<()> {
        if byte_index >= self.len() {
            return None;
        }
        let storage_index = self.window.start.checked_add(byte_index)?;
        if let Some(bytes) = Arc::get_mut(&mut self.backing) {
            *bytes.get_mut(storage_index)? = value;
        } else {
            let mut bytes = self.bytes().to_vec();
            *bytes.get_mut(byte_index)? = value;
            *self = Self::new(bytes);
        }
        Some(())
    }

    pub(crate) fn subslice(&self, start: usize, end: usize) -> Option<Self> {
        self.bytes().get(start..end)?;
        Some(Self {
            backing: Arc::clone(&self.backing),
            window: self.window.start.checked_add(start)?..self.window.start.checked_add(end)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::byte_sequences::ByteSequenceView;
    use std::sync::Arc;

    #[test]
    fn invalid_replacement_does_not_detach_or_change_length() {
        let source = ByteSequenceView::new(vec![1, 2]);
        let mut destination = source.clone();
        for byte_index in [2, usize::MAX] {
            assert_eq!(destination.replace_byte(byte_index, 9), None);
            assert!(Arc::ptr_eq(&source.backing, &destination.backing));
            assert_eq!(destination.bytes(), &[1, 2]);
        }
        let mut empty = source.subslice(1, 1).unwrap();
        assert_eq!(empty.replace_byte(0, 9), None);
        assert_eq!(empty.len(), 0);
    }
}
