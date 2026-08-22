//! Direct and one-alias landed-literal affine evidence reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody;

pub(super) fn retained_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    if facts()
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
                    [
                        Proposition::LessOrEqual(literal.clone(), root.clone()),
                        Proposition::LessOrEqual(root.clone(), literal.clone()),
                    ]
                    .iter()
                    .any(|root_bound| {
                        affine_custody::retained_from_root(
                            context,
                            goal,
                            semantic_axioms,
                            root,
                            root_bound,
                        )
                    })
                })
        })
    {
        return true;
    }

    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter(|(root, alias)| {
                    root != alias
                        && matches!(root, ScalarTerm::Value { .. })
                        && matches!(alias, ScalarTerm::Value { .. })
                        && root.scalar_type() == alias.scalar_type()
                })
                .any(|(root, alias)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let literal = if inner_left == alias {
                                inner_right
                            } else if inner_right == alias {
                                inner_left
                            } else {
                                return false;
                            };
                            let Some((integer_type, _)) = literal.integer_value() else {
                                return false;
                            };
                            if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                                return false;
                            }
                            [
                                Proposition::LessOrEqual(literal.clone(), root.clone()),
                                Proposition::LessOrEqual(root.clone(), literal.clone()),
                            ]
                            .iter()
                            .any(|root_bound| {
                                affine_custody::retained_from_root(
                                    context,
                                    goal,
                                    semantic_axioms,
                                    root,
                                    root_bound,
                                )
                            })
                        })
                })
        })
}
