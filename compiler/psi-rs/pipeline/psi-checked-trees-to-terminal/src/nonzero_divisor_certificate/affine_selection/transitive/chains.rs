//! Ordered exact two-citation chains for affine certificate production.

use psi_core::Proposition;

use super::super::super::integer_evidence::Citation;

mod left_legs;
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
        left_legs::find(
            self.assumptions,
            self.semantic_axioms,
            |left_citation, left_fact, middle| {
                for &(right_citation, right_fact) in self.right_legs.candidates(middle) {
                    if std::ptr::eq(left_fact, right_fact) {
                        continue;
                    }
                    if let Some(result) =
                        complete(left_citation, left_fact, right_citation, right_fact)
                    {
                        return Some(result);
                    }
                }
                None
            },
        )
    }
}
