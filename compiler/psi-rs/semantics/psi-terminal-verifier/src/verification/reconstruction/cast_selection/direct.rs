//! Independent direct retained integer-cast root-bound selection.

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
