//! Independent fixed-depth value-alias selection for obligation reconstruction.
//!
//! These selectors deliberately mirror, rather than share with, the untrusted
//! producer. Their one- and two-alias entry points expose no generic depth or
//! recursive graph search.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::{
    closed_integer_less_or_equal, retained_cast_bound_from_root, retained_remap_integer_literal,
};

pub(super) fn retained_one(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    let bounds_by_endpoint = indexed_bounds(requirements, semantic_axioms);
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
                .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
                .any(|(root, alias)| {
                    bounds_by_endpoint.get(alias).is_some_and(|bounds| {
                        bounds.iter().any(|(relation, endpoint)| {
                            let root_bound = substitute_bound_endpoint(relation, root, *endpoint);
                            complete(root, &root_bound)
                        })
                    })
                })
        })
}

pub(super) fn retained_two(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    let bounds_by_endpoint = indexed_bounds(requirements, semantic_axioms);
    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter(|(root, middle_alias)| distinct_same_carrier_values(root, middle_alias))
                .any(|(root, middle_alias)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let bound_alias = if inner_left == middle_alias {
                                inner_right
                            } else if inner_right == middle_alias {
                                inner_left
                            } else {
                                return false;
                            };
                            if bound_alias == root
                                || !distinct_same_carrier_values(middle_alias, bound_alias)
                            {
                                return false;
                            }
                            bounds_by_endpoint.get(bound_alias).is_some_and(|bounds| {
                                bounds.iter().any(|(relation, endpoint)| {
                                    let root_bound =
                                        substitute_bound_endpoint(relation, root, *endpoint);
                                    complete(root, &root_bound)
                                })
                            })
                        })
                })
        })
}

pub(super) fn retained_stronger_cast(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
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
            .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
            .any(|(root, alias)| {
                facts()
                    .filter_map(|bound| match bound {
                        Proposition::LessOrEqual(left, right) => Some((left, right)),
                        _ => None,
                    })
                    .any(|(bound_left, bound_right)| {
                        let (retained_literal, endpoint) = if bound_left == alias {
                            (bound_right, 0)
                        } else if bound_right == alias {
                            (bound_left, 1)
                        } else {
                            return false;
                        };
                        let Some((integer_type, _)) = retained_literal.integer_value() else {
                            return false;
                        };
                        root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                            && retained_cast_from_stronger_bound(
                                context,
                                goal,
                                semantic_axioms,
                                root,
                                retained_literal,
                                endpoint,
                            )
                    })
            })
        })
}

fn retained_cast_from_stronger_bound(
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
    let Some(source_endpoint) = retained_remap_integer_literal(target_endpoint, root_type) else {
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
    retained_cast_bound_from_root(context, goal, semantic_axioms, root, &root_bound)
}

pub(super) fn retained_landed_literal_cast(
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
            let Some(source_endpoint) = retained_remap_integer_literal(target_endpoint, root_type)
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
            retained_cast_bound_from_root(context, goal, semantic_axioms, root, &root_bound)
        })
}

fn indexed_bounds<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(&'a Proposition, usize)>> {
    let mut bounds_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for fact in requirements.iter().chain(semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((fact, 0));
        }
        if right != left && matches!(right, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((fact, 1));
        }
    }
    bounds_by_endpoint
}

fn distinct_same_carrier_values(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    left != right
        && matches!(left, ScalarTerm::Value { .. })
        && matches!(right, ScalarTerm::Value { .. })
        && left.scalar_type() == right.scalar_type()
}

fn substitute_bound_endpoint(
    relation: &Proposition,
    replacement: &ScalarTerm,
    endpoint: usize,
) -> Proposition {
    let Proposition::LessOrEqual(left, right) = relation else {
        unreachable!("only order bounds are indexed")
    };
    if endpoint == 0 {
        Proposition::LessOrEqual(replacement.clone(), right.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), replacement.clone())
    }
}
