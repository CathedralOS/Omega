//! Ordered exact two-citation chains for affine certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_admission::ProofNode;

use super::super::fact_identity;

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
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode, ProofNode) -> Option<T>,
    ) -> Option<T> {
        left_legs::find(
            self.assumptions,
            self.semantic_axioms,
            |left_citation, left_fact, left, middle| {
                self.right_legs.candidates(middle).iter().find_map(
                    |&(right_citation, right_fact, right)| {
                        if !fact_identity::distinct(left_fact, right_fact) {
                            return None;
                        }
                        complete(
                            left,
                            right,
                            left_citation.proof(left_fact),
                            right_citation.proof(right_fact),
                        )
                    },
                )
            },
        )
    }
}
