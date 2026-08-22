//! Alias-landed literals for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::cast_custody;
use super::super::super::integer_evidence::closed_integer_less_or_equal;
use super::super::distinct_same_carrier_values;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    facts()
        .filter_map(|root_equality| match root_equality {
            Proposition::Equal(left, right) => Some((root_equality, left, right)),
            _ => None,
        })
        .any(|(root_equality, root_left, root_right)| {
            [(root_left, root_right), (root_right, root_left)]
                .into_iter()
                .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
                .any(|(root, alias)| {
                    facts()
                        .filter(|literal_equality| !std::ptr::eq(root_equality, *literal_equality))
                        .filter_map(|literal_equality| match literal_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(literal_left, literal_right)| {
                            let literal = if literal_left == alias {
                                literal_right
                            } else if literal_right == alias {
                                literal_left
                            } else {
                                return false;
                            };
                            let Some((integer_type, _)) = literal.integer_value() else {
                                return false;
                            };
                            root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                                && retained_cast_from_landed_literal(
                                    context,
                                    goal,
                                    semantic_axioms,
                                    root,
                                    literal,
                                )
                        })
                })
        })
}

fn retained_cast_from_landed_literal(
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
