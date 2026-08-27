//! Independent exact integer-cast witness and bound-conversion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

mod target;

pub(in super::super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .any(|candidate| {
            target::retained(context, goal, semantic_axioms, root, root_bound, candidate)
        })
}
