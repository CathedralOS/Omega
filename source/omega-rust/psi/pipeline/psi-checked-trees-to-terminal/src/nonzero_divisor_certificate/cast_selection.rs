//! Side-local evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

mod affine;
mod alias;
mod direct;
mod literal;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    literal::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| alias::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| affine::prove(context, goal, assumptions, semantic_axioms))
}
