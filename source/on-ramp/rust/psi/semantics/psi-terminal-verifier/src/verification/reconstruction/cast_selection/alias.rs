//! Fixed alias-family reconstruction for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};

use super::super::alias_transport;

mod one;
mod two;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if one::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    alias_transport::retained_stronger_cast(context, goal, requirements, semantic_axioms)
        || alias_transport::retained_landed_literal_cast(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || two::retained(context, goal, requirements, semantic_axioms)
}
