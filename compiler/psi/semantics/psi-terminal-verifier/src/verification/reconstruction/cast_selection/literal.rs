//! Direct landed-literal custody for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext};

mod candidates;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    candidates::retained(context, goal, requirements, semantic_axioms)
}
