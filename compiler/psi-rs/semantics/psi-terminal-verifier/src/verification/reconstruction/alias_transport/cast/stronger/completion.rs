//! Independent typed stronger alias-bound completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::cast_custody;
use super::super::super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    retained_literal: &ScalarTerm,
    endpoint: usize,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return false;
    };
    let (target, target_endpoint) = if endpoint == 1 {
        (goal_right, goal_left)
    } else {
        (goal_left, goal_right)
    };
    if !matches!(target, ScalarTerm::Value { .. }) {
        return false;
    }
    let Some(source_endpoint) = cast_custody::remap_integer_literal(target_endpoint, root_type)
    else {
        return false;
    };
    let closed = if endpoint == 1 {
        closed_integer_less_or_equal(&source_endpoint, retained_literal)
    } else {
        closed_integer_less_or_equal(retained_literal, &source_endpoint)
    };
    if !closed {
        return false;
    }
    let root_bound = if endpoint == 1 {
        Proposition::LessOrEqual(source_endpoint, root.clone())
    } else {
        Proposition::LessOrEqual(root.clone(), source_endpoint)
    };
    cast_custody::retained_from_root(context, goal, semantic_axioms, root, &root_bound)
}
