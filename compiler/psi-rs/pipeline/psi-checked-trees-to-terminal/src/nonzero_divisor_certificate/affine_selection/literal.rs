//! Direct and one-alias landed-literal affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod alias;
mod direct;

pub(super) fn prove_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    alias::prove(context, goal, assumptions, semantic_axioms)
}
