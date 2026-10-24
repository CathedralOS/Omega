use std::fmt;
use std::marker::PhantomData;

use crate::Handle;

pub struct HandleSpan<T> {
    start: Handle<T>,
    count: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> HandleSpan<T> {
    pub const fn empty() -> Self {
        Self {
            start: Handle::invalid(),
            count: 0,
            marker: PhantomData,
        }
    }

    pub const fn from_parts(start: Handle<T>, count: u32) -> Self {
        Self {
            start,
            count,
            marker: PhantomData,
        }
    }

    /// The span covering `count` handles from `start`, or the empty span —
    /// with its invalid start — when `count` is zero.
    pub const fn from_parts_or_empty(start: Handle<T>, count: u32) -> Self {
        if count == 0 {
            Self::empty()
        } else {
            Self::from_parts(start, count)
        }
    }

    pub const fn start(self) -> Handle<T> {
        self.start
    }

    pub const fn count(self) -> u32 {
        self.count
    }

    pub const fn len(self) -> usize {
        self.count as usize
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    /// The handle `offset` positions into the span.
    pub fn handle_at(self, offset: u32) -> Handle<T> {
        Handle::from_parts(
            self.start
                .arena_index()
                .checked_add(offset)
                .expect("handle span index overflow"),
            self.start.generation(),
        )
    }

    /// Iterates the span's handles in order: `start` through `start + count`.
    ///
    /// This is the one owner of the span-to-handles rule. A caller that keeps
    /// the iterator — or needs `len`/`nth` over the sequence — uses the same
    /// index arithmetic as `handle_at` instead of reimplementing the walk.
    pub fn handles(self) -> SpanHandles<T> {
        SpanHandles {
            span: self,
            offset: 0,
        }
    }

    /// Whether `handle` is the contiguous next handle after this span: same
    /// generation, arena index exactly `start + count`.
    pub fn is_followed_by(self, handle: Handle<T>) -> bool {
        !self.is_empty()
            && handle.generation() == self.start.generation()
            && handle.arena_index() == self.start.arena_index() + self.count
    }

    pub fn push_contiguous(&mut self, handle: Handle<T>) {
        if self.is_empty() {
            *self = Self::from_parts(handle, 1);
            return;
        }

        let expected_index = self
            .start
            .arena_index()
            .checked_add(self.count)
            .expect("handle span index overflow");
        assert_eq!(
            handle.arena_index(),
            expected_index,
            "handle span push must be contiguous"
        );
        assert_eq!(
            handle.generation(),
            self.start.generation(),
            "handle span push must keep one generation"
        );
        self.count = self
            .count
            .checked_add(1)
            .expect("handle span count overflow");
    }
}

/// The handles of one span, in order: `start` through `start + count`.
///
/// The sequence is a span plus a consumed offset, so `len` and `nth` skip by
/// index arithmetic instead of stepping, and every handle keeps the span's
/// generation. An exhausted iterator stays exhausted.
#[derive(Debug)]
pub struct SpanHandles<T> {
    span: HandleSpan<T>,
    offset: u32,
}

impl<T> SpanHandles<T> {
    pub const fn empty() -> Self {
        Self {
            span: HandleSpan::empty(),
            offset: 0,
        }
    }

    #[inline]
    fn remaining(&self) -> u32 {
        self.span.count() - self.offset
    }
}

impl<T> Clone for SpanHandles<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SpanHandles<T> {}

impl<T> Iterator for SpanHandles<T> {
    type Item = Handle<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.span.count() {
            return None;
        }

        let handle = self.span.handle_at(self.offset);
        self.offset += 1;

        Some(handle)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining() as usize;
        (remaining, Some(remaining))
    }

    /// Skip whole parts by length: the span knows its count, so skipping is
    /// index arithmetic rather than `n` calls to `next`.
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match u32::try_from(n) {
            Ok(skip) if skip < self.remaining() => {
                self.offset += skip;
                self.next()
            }
            _ => {
                self.offset = self.span.count();
                None
            }
        }
    }
}

impl<T> ExactSizeIterator for SpanHandles<T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining() as usize
    }
}

impl<T> std::iter::FusedIterator for SpanHandles<T> {}

impl<T> Clone for HandleSpan<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HandleSpan<T> {}

impl<T> Default for HandleSpan<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> PartialEq for HandleSpan<T> {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.count == other.count
    }
}

impl<T> Eq for HandleSpan<T> {}

/// Hashes exactly what `PartialEq` compares, so a span can key a set without
/// requiring `T` to be hashable -- a span holds a start handle and a count,
/// never a `T`. `Handle` carries the same pair of impls for the same reason.
impl<T> std::hash::Hash for HandleSpan<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.count.hash(state);
    }
}

impl<T> fmt::Debug for HandleSpan<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandleSpan")
            .field("start", &self.start)
            .field("count", &self.count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Handle, HandleSpan};

    #[test]
    fn pushes_contiguous_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_arena_index(5));

        assert_eq!(span.start(), Handle::from_arena_index(4));
        assert_eq!(span.count(), 2);
    }

    #[test]
    #[should_panic(expected = "handle span push must be contiguous")]
    fn panics_on_non_contiguous_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_arena_index(6));
    }

    #[test]
    #[should_panic(expected = "handle span push must keep one generation")]
    fn panics_on_mixed_generation_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_parts(5, 2));
    }
}
