//! Ordered exact two-citation chains for independent affine reconstruction.

use psi_core::Proposition;

mod left_legs;
mod right_index;

use left_legs::LeftLegs;
use right_index::RightLegIndex;

pub(super) struct TwoCitationChains<'a> {
    left_legs: LeftLegs<'a>,
    right_legs: RightLegIndex<'a>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            left_legs: LeftLegs::new(requirements, semantic_axioms),
            right_legs: RightLegIndex::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a Proposition, &'a Proposition) -> bool,
    ) -> bool {
        self.left_legs.any(|left_fact, middle| {
            for &right_fact in self.right_legs.candidates(middle) {
                if std::ptr::eq(left_fact, right_fact) {
                    continue;
                }
                if complete(left_fact, right_fact) {
                    return true;
                }
            }
            false
        })
    }
}
