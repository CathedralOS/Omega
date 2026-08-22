//! Direct landed-literal custody for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::cast_custody;
use super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(left, right)| {
            [(left, right), (right, left)]
                .into_iter()
                .filter(|(root, literal)| {
                    matches!(root, ScalarTerm::Value { .. })
                        && literal.integer_value().is_some_and(|(integer_type, _)| {
                            root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                        })
                })
                .any(|(root, literal)| {
                    retained_cast_bound_from_landed_literal(
                        context,
                        goal,
                        semantic_axioms,
                        root,
                        literal,
                    )
                })
        })
}

fn retained_cast_bound_from_landed_literal(
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
            let closed = if endpoint == 1 {
                closed_integer_less_or_equal(&source_endpoint, landed_literal)
            } else {
                closed_integer_less_or_equal(landed_literal, &source_endpoint)
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
        })
}
