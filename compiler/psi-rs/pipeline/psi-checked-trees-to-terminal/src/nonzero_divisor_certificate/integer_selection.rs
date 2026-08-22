//! Canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod bound;
mod dispatch;
mod exact;
mod logical;
mod order;
mod substitution;

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if let Some(proof) = exact::prove(goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    dispatch::prove(context, goal, assumptions, semantic_axioms, |part| {
        build(context, part, assumptions, semantic_axioms)
    })
}
