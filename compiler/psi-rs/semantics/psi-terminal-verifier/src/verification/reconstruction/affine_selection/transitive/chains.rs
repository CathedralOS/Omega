//! Ordered exact two-citation chains for independent affine reconstruction.

use psi_core::Proposition;

mod left_legs;
mod right_index;

use right_index::RightLegIndex;

pub(super) struct TwoCitationChains<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    right_legs: RightLegIndex<'a>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
            right_legs: RightLegIndex::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a Proposition, &'a Proposition) -> bool,
    ) -> bool {
        left_legs::any(
            self.requirements,
            self.semantic_axioms,
            |left_fact, middle| {
                for &right_fact in self.right_legs.candidates(middle) {
                    if std::ptr::eq(left_fact, right_fact) {
                        continue;
                    }
                    if complete(left_fact, right_fact) {
                        return true;
                    }
                }
                false
            },
        )
    }
}
