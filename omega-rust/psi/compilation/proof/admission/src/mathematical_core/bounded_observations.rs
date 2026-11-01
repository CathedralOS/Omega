//! Test observations of actual bounded operations on the current thread.
//!
//! A spawned worker has separate storage, so the public-entry tests cannot be
//! satisfied by returning a result from a hidden wide-stack invocation.

thread_local! {
    static ENTRIES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

pub(crate) fn bounded_verification_entries() -> (usize, usize) {
    ENTRIES.get()
}

pub(crate) fn record_admission() {
    ENTRIES.update(|(admission, denotation)| (admission + 1, denotation));
}

pub(super) fn record_denotation() {
    ENTRIES.update(|(admission, denotation)| (admission, denotation + 1));
}
