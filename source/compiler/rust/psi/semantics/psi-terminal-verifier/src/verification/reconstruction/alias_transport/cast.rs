//! Cast-specific alias reconstruction facade.

use psi_core::{Proposition, PropositionContext};

mod literal;
mod stronger;

pub(in super::super) fn retained_stronger_cast(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    stronger::retained(context, goal, requirements, semantic_axioms)
}

pub(in super::super) fn retained_landed_literal_cast(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    literal::retained(context, goal, requirements, semantic_axioms)
}
