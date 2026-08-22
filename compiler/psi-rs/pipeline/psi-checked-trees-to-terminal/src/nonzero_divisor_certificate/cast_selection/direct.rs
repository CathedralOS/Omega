//! Direct retained integer-cast root-bound proof selection.

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
