//! Independent fixed two-equality affine endpoint reconstruction.

use psi_core::{Proposition, PropositionContext};

mod selection;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    selection::retained(context, goal, requirements, semantic_axioms)
}
