//! Independent canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};

mod bound;
mod dispatch;
mod exact;
mod logical;
mod order;
mod substitution;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if exact::retained(goal, requirements, semantic_axioms) {
        return true;
    }
    dispatch::retained(context, goal, requirements, semantic_axioms, |part| {
        retained(context, part, requirements, semantic_axioms)
    })
}
