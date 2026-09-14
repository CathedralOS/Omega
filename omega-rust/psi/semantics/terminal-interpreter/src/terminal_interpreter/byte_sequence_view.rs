//! Invocation-private byte backing with immutable shared views.
//! Storage identity never enters Terminal bytes.

use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct ByteSequenceView {
    backing: Arc<[u8]>,
    window: Range<usize>,
}

impl ByteSequenceView {
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        let length = bytes.len();
        Self {
            backing: bytes.into(),
            window: 0..length,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.backing[self.window.clone()]
    }

    pub(super) fn get(&self, index: usize) -> Option<&u8> {
        self.bytes().get(index)
    }

    /// Replace one live byte without changing another view's immutable value.
    /// Shared backing detaches only this window, not unused source bytes.
    pub(super) fn replace_byte(&mut self, byte_index: usize, value: u8) -> Option<()> {
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

    pub(super) fn subslice(&self, start: usize, end: usize) -> Option<Self> {
        self.bytes().get(start..end)?;
        Some(Self {
            backing: Arc::clone(&self.backing),
            window: self.window.start.checked_add(start)?..self.window.start.checked_add(end)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_tails_and_call_clones_share_backing_without_exposing_other_bytes() {
        let source = ByteSequenceView::new(vec![0, 128, 255, 7]);
        let call = source.clone();
        let tail = call.subslice(1, 3).unwrap();
        let nested = tail.subslice(1, 2).unwrap();
        assert!(Arc::ptr_eq(&source.backing, &nested.backing));
        assert_eq!(tail.bytes(), &[128, 255]);
        assert_eq!(nested.bytes(), &[255]);
        assert_eq!(tail.subslice(2, 2).unwrap().bytes(), &[]);
        assert!(tail.subslice(0, 3).is_none());
        assert!(tail.subslice(2, 1).is_none());
        assert!(tail.subslice(0, usize::MAX).is_none());
        drop(source);
        drop(call);
        assert_eq!(nested.bytes(), &[255]);
    }

    #[test]
    fn replacement_detaches_shared_windows_and_reuses_exclusive_backing() {
        let source = ByteSequenceView::new(vec![10, 20, 30, 40]);
        let sibling = source.clone();
        let mut destination = source.subslice(1, 3).unwrap();
        assert_eq!(destination.replace_byte(1, 255), Some(()));
        assert_eq!(source.bytes(), &[10, 20, 30, 40]);
        assert_eq!(sibling.bytes(), source.bytes());
        assert_eq!(destination.bytes(), &[20, 255]);
        assert_eq!(destination.backing.len(), 2);
        let backing = destination.bytes().as_ptr();
        assert_eq!(destination.replace_byte(0, 0), Some(()));
        assert_eq!(destination.bytes().as_ptr(), backing);
        assert_eq!(destination.bytes(), &[0, 255]);
    }

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
