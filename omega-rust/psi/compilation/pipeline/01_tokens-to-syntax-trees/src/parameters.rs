//! Parameter lists: `parse_parameters` reads ordinary parameters and return
//! types, `parse_generic_parameters` the generic parameter syntax,
//! `binding_properties` the relevance brackets on a binding and `contracts`
//! the contracts attached to a machine parameter.

pub(crate) mod binding_properties;
pub(crate) mod contracts;
#[cfg(test)]
mod generic_parameters_tests;
pub(crate) mod parse_generic_parameters;
pub(crate) mod parse_parameters;

/// Tracks the first handle and element count of a contiguous parameter span
/// while parameters are appended one at a time.
pub(crate) struct ParameterSpanBuilder<T> {
    start: arena::Handle<T>,
    count: u32,
}

impl<T> ParameterSpanBuilder<T> {
    pub(crate) fn new() -> Self {
        Self {
            start: arena::Handle::invalid(),
            count: 0,
        }
    }

    pub(crate) fn push(&mut self, handle: arena::Handle<T>) {
        if self.count == 0 {
            self.start = handle;
        }
        self.count = self
            .count
            .checked_add(1)
            .expect("parameter span count overflow");
    }

    pub(crate) fn finish(self) -> arena::HandleSpan<T> {
        arena::HandleSpan::from_parts_or_empty(self.start, self.count)
    }
}
