thread_local! {
    static ENTRIES: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

pub(crate) fn bounded_verification_entries() -> (usize, usize) {
    ENTRIES.get()
}

pub(crate) fn record_admission() {
    ENTRIES.update(|(admission, denotation)| (admission + 1, denotation));
}

pub(crate) fn record_denotation() {
    ENTRIES.update(|(admission, denotation)| (admission, denotation + 1));
}
