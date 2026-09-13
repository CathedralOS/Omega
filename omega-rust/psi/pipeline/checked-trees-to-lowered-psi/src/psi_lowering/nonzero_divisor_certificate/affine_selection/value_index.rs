//! Source-ordered Value-keyed candidate storage for certificate production.

use std::collections::BTreeMap;

use semantic_vocabulary::ScalarTerm;

pub(super) struct ValueIndex<T> {
    by_value: BTreeMap<ScalarTerm, Vec<T>>,
}

impl<T> ValueIndex<T> {
    pub(super) fn new() -> Self {
        Self {
            by_value: BTreeMap::new(),
        }
    }

    pub(super) fn push(&mut self, value: &ScalarTerm, candidate: T) {
        self.by_value
            .entry(value.clone())
            .or_default()
            .push(candidate);
    }

    pub(super) fn candidates(&self, value: &ScalarTerm) -> &[T] {
        self.by_value.get(value).map(Vec::as_slice).unwrap_or(&[])
    }
}
