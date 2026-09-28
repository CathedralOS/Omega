//! Source-ordered right-leg index for affine certificate production.

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::Citation;
use super::super::super::{bounds, value_index::ValueIndex};

pub(super) struct RightLegIndex<'a> {
    by_left_endpoint: ValueIndex<(Citation, &'a Proposition, &'a ScalarTerm)>,
}

impl<'a> RightLegIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_left_endpoint = ValueIndex::new();
        for (citation, fact, left, right) in bounds::with_value_left(assumptions, semantic_axioms) {
            by_left_endpoint.push(left, (citation, fact, right));
        }
        Self { by_left_endpoint }
    }

    pub(super) fn candidates(
        &self,
        left_endpoint: &ScalarTerm,
    ) -> &[(Citation, &'a Proposition, &'a ScalarTerm)] {
        self.by_left_endpoint.candidates(left_endpoint)
    }
}
