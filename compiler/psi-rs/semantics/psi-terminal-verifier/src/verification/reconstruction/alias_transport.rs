//! Independent fixed-depth value-alias selection for obligation reconstruction.
//!
//! These selectors deliberately mirror, rather than share with, the untrusted
//! producer. Their one- and two-alias entry points expose no generic depth or
//! recursive graph search.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

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
