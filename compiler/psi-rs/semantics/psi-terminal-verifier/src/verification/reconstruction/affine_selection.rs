//! Side-local selection of retained evidence for bounded affine reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::{affine_custody, alias_transport};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(left, right) => Some((fact, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            [root_left, root_right]
                .into_iter()
                .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                .any(|root| {
                    affine_custody::retained_from_root(
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
    retained_landed_literal_affine_bound(context, goal, requirements, semantic_axioms)
        || retained_alias_substituted_affine_bound(context, goal, requirements, semantic_axioms)
        || retained_transitively_reconstructed_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_transitively_alias_substituted_affine_bound(
            context,
            goal,
            requirements,
            semantic_axioms,
        )
        || retained_two_alias_substituted_affine_bound(context, goal, requirements, semantic_axioms)
}

fn retained_landed_literal_affine_bound(
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

fn retained_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_one(requirements, semantic_axioms, |root, root_bound| {
        affine_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}

fn retained_two_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    alias_transport::retained_two(requirements, semantic_axioms, |root, root_bound| {
        affine_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}

fn retained_transitively_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
    for fact in facts() {
        let Proposition::LessOrEqual(left, _) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_left_endpoint
                .entry(left.clone())
                .or_default()
                .push(fact);
        }
    }

    facts()
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(equality_left, equality_right)| {
            [
                (equality_left, equality_right),
                (equality_right, equality_left),
            ]
            .into_iter()
            .filter(|(root, alias)| {
                root != alias
                    && matches!(root, ScalarTerm::Value { .. })
                    && matches!(alias, ScalarTerm::Value { .. })
            })
            .any(|(root, alias)| {
                facts().any(|left_fact| {
                    let Proposition::LessOrEqual(left, middle) = left_fact else {
                        return false;
                    };
                    if !matches!(middle, ScalarTerm::Value { .. }) {
                        return false;
                    }
                    bounds_by_left_endpoint
                        .get(middle)
                        .is_some_and(|right_facts| {
                            right_facts.iter().any(|right_fact| {
                                if std::ptr::eq(left_fact, *right_fact) {
                                    return false;
                                }
                                let Proposition::LessOrEqual(_, right) = right_fact else {
                                    unreachable!("only integer bounds are indexed")
                                };
                                let root_bound = if alias == left {
                                    Proposition::LessOrEqual(root.clone(), right.clone())
                                } else if alias == right {
                                    Proposition::LessOrEqual(left.clone(), root.clone())
                                } else {
                                    return false;
                                };
                                affine_custody::retained_from_root(
                                    context,
                                    goal,
                                    semantic_axioms,
                                    root,
                                    &root_bound,
                                )
                            })
                        })
                })
            })
        })
}

fn retained_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
    for fact in facts() {
        let Proposition::LessOrEqual(left, _) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_left_endpoint
                .entry(left.clone())
                .or_default()
                .push(fact);
        }
    }

    facts().any(|left_fact| {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            return false;
        };
        if !matches!(middle, ScalarTerm::Value { .. }) {
            return false;
        }
        bounds_by_left_endpoint
            .get(middle)
            .is_some_and(|right_facts| {
                right_facts.iter().any(|right_fact| {
                    if std::ptr::eq(left_fact, *right_fact) {
                        return false;
                    }
                    let Proposition::LessOrEqual(_, right) = right_fact else {
                        unreachable!("only integer bounds are indexed")
                    };
                    let root_bound = Proposition::LessOrEqual(left.clone(), right.clone());
                    [left, right]
                        .into_iter()
                        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                        .any(|root| {
                            affine_custody::retained_from_root(
                                context,
                                goal,
                                semantic_axioms,
                                root,
                                &root_bound,
                            )
                        })
                })
            })
    })
}
