//! Ordered exact two-citation chains for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

mod right_index;

use right_index::RightLegIndex;

pub(super) struct TwoCitationChains<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    right_legs: RightLegIndex<'a>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
            right_legs: RightLegIndex::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(Citation, &'a Proposition, Citation, &'a Proposition) -> Option<T>,
    ) -> Option<T> {
        for (left_citation, left_fact) in cited_facts(self.assumptions, self.semantic_axioms) {
            let Proposition::LessOrEqual(_, middle) = left_fact else {
                continue;
            };
            if !matches!(middle, ScalarTerm::Value { .. }) {
                continue;
            }
            for &(right_citation, right_fact) in self.right_legs.candidates(middle) {
                if std::ptr::eq(left_fact, right_fact) {
                    continue;
                }
                if let Some(result) = complete(left_citation, left_fact, right_citation, right_fact)
                {
                    return Some(result);
                }
            }
        }
        None
    }
}
