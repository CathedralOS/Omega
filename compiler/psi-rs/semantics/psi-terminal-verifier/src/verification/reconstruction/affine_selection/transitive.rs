//! Fixed two-citation transitive affine evidence reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody;

pub(super) fn retained_transitively_alias_substituted_affine_bound(
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

pub(super) fn retained_transitively_reconstructed_affine_bound(
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
