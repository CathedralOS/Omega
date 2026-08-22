//! Source-ordered right-leg index for independent two-citation reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

pub(super) struct RightLegIndex<'a> {
    by_left_endpoint: BTreeMap<ScalarTerm, Vec<&'a Proposition>>,
}

impl<'a> RightLegIndex<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
        for fact in requirements.iter().chain(semantic_axioms) {
            let Proposition::LessOrEqual(left, _) = fact else {
                continue;
            };
            if matches!(left, ScalarTerm::Value { .. }) {
                by_left_endpoint.entry(left.clone()).or_default().push(fact);
            }
        }
        Self { by_left_endpoint }
    }

    pub(super) fn candidates(&self, left_endpoint: &ScalarTerm) -> &[&'a Proposition] {
        self.by_left_endpoint
            .get(left_endpoint)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
