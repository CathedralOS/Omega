//! Independent typed landed-literal alias completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::cast_custody;

mod bound;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return false;
    };
    [(goal_right, goal_left, 1), (goal_left, goal_right, 0)]
        .into_iter()
        .filter(|(target, _, _)| matches!(target, ScalarTerm::Value { .. }))
        .any(|(_, target_endpoint, endpoint)| {
            let Some(source_endpoint) =
                cast_custody::remap_integer_literal(target_endpoint, root_type)
            else {
                return false;
            };
            let Some(root_bound) = bound::retained(root, landed_literal, source_endpoint, endpoint)
            else {
                return false;
            };
            cast_custody::retained_from_root(context, goal, semantic_axioms, root, &root_bound)
        })
}
