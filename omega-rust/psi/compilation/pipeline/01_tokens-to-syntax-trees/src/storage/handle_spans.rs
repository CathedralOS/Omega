//! Contiguous arena handle spans. Every grammar table that appends a run of
//! handles while staging the span it will return tracks the run through the one
//! staging owner here, so the first-handle/count bookkeeping and its overflow
//! guard are spelled once instead of at each appending site.

use arena::{Handle, HandleSpan};

/// Tracks the first handle and element count of one contiguous arena span
/// while its elements are appended one at a time.
pub(crate) struct HandleSpanBuilder<T> {
    start: Handle<T>,
    count: u32,
}

impl<T> HandleSpanBuilder<T> {
    pub(crate) fn new() -> Self {
        Self {
            start: Handle::invalid(),
            count: 0,
        }
    }

    /// Records the handle just appended to the span's table.
    pub(crate) fn push(&mut self, handle: Handle<T>) {
        if self.count == 0 {
            self.start = handle;
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("handle span count overflow");
    }

    /// Records a contiguous run just appended to the span's table.
    pub(crate) fn push_span(&mut self, span: HandleSpan<T>) {
        if span.is_empty() {
            return;
        }
        if self.count == 0 {
            self.start = span.start();
        }
        self.count = self
            .count
            .checked_add(span.count())
            .expect("handle span count overflow");
    }

    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn finish(self) -> HandleSpan<T> {
        HandleSpan::from_parts_or_empty(self.start, self.count)
    }
}
