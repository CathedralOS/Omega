use std::fmt;
use std::marker::PhantomData;

use crate::arena::Handle;

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

    pub const fn is_empty(self) -> bool {
        self.count == 0
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
