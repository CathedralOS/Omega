//! Source-ordered right-leg index for independent two-citation reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::{bounds, value_index::ValueIndex};

pub(super) struct RightLegIndex<'a> {
    by_left_endpoint: ValueIndex<(&'a Proposition, &'a ScalarTerm)>,
}

impl<'a> RightLegIndex<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_left_endpoint = ValueIndex::new();
        for (fact, left, right) in bounds::with_value_left(requirements, semantic_axioms) {
            by_left_endpoint.push(left, (fact, right));
        }
        Self { by_left_endpoint }
    }

    pub(super) fn candidates(
        &self,
        left_endpoint: &ScalarTerm,
    ) -> &[(&'a Proposition, &'a ScalarTerm)] {
        self.by_left_endpoint.candidates(left_endpoint)
    }
}
