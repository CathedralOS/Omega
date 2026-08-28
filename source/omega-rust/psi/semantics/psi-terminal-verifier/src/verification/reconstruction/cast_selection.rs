//! Side-local retained-evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};

mod affine;
mod alias;
mod direct;
mod literal;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return false;
    }
    if direct::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    if literal::retained(context, goal, requirements, semantic_axioms) {
        return true;
    }
    alias::retained(context, goal, requirements, semantic_axioms)
        || affine::retained(context, goal, requirements, semantic_axioms)
}
