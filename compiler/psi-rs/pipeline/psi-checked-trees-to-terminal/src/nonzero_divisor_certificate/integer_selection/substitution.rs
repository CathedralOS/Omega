//! Fixed one- and two-equality integer-bound substitution proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod one;
mod relation;
mod two;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    one::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| two::prove(context, goal, assumptions, semantic_axioms))
}
