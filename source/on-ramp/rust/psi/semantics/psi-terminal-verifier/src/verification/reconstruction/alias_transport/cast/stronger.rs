//! Closed-strengthened alias bounds for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext};

mod candidates;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    candidates::any(
        requirements,
        semantic_axioms,
        |root, retained_literal, endpoint| {
            completion::retained(
                context,
                goal,
                semantic_axioms,
                root,
                retained_literal,
                endpoint,
            )
        },
    )
}
