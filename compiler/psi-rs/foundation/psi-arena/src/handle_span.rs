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
