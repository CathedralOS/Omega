//! Direct retained integer-cast root-bound proof selection.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

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
