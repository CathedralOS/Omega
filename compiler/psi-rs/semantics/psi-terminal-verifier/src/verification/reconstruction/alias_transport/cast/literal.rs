//! Alias-landed literals for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext};

mod candidates;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    candidates::any(requirements, semantic_axioms, |root, landed_literal| {
        completion::retained(context, goal, semantic_axioms, root, landed_literal)
    })
}
