//! Fixed two-equality affine endpoint substitution.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod selection;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    selection::prove(context, goal, assumptions, semantic_axioms)
}
