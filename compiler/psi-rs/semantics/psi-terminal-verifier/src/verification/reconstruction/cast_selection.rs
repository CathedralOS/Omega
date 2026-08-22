//! Side-local retained-evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::integer_evidence::closed_integer_less_or_equal;
use super::{alias_transport, cast_custody};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return false;
    }
    if requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|root_bound| match root_bound {
            Proposition::LessOrEqual(left, right) => Some((root_bound, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            [root_left, root_right]
                .into_iter()
                .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                .any(|root| {
                    cast_custody::retained_from_root(
                        context,
                        goal,
                        semantic_axioms,
                        root,
                        root_bound,
                    )
                })
        })
    {
        return true;
    }
    if requirements
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
    {
        return true;
    }
    retained_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    }) {
        return true;
    }
    alias_transport::retained_stronger_cast(context, goal, requirements, semantic_axioms)
        || alias_transport::retained_landed_literal_cast(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_two_alias_substituted_cast_bound(context, goal, requirements, semantic_axioms)
}

fn retained_two_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
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
