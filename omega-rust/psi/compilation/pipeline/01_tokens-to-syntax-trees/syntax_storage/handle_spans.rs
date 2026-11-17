use arena::{Handle, HandleSpan};

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

    pub(crate) fn push(&mut self, handle: Handle<T>) {
        if self.count == 0 {
            self.start = handle;
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("handle span count overflow");
    }

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

    pub(crate) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(crate) fn finish(self) -> HandleSpan<T> {
        HandleSpan::from_parts_or_empty(self.start, self.count)
    }
}
