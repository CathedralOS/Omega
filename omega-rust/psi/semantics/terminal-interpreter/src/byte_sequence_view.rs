//! Invocation-private immutable storage. Identity never enters Terminal bytes.

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
}
