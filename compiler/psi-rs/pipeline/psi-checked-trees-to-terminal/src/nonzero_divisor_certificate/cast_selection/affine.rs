//! Affine-root custody for one following exact partial-cast spine.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod candidates;
mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    candidates::prove(context, goal, assumptions, semantic_axioms)
}
