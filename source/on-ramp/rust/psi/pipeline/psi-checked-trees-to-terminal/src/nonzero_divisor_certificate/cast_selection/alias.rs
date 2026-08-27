//! Fixed alias-family dispatch for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::alias_transport;

mod one;
mod two;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    one::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| {
            alias_transport::prove_stronger_cast(context, goal, assumptions, semantic_axioms)
        })
        .or_else(|| {
            alias_transport::prove_landed_literal_cast(context, goal, assumptions, semantic_axioms)
        })
        .or_else(|| two::prove(context, goal, assumptions, semantic_axioms))
}
