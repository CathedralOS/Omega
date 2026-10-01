//! Fixed-depth value-alias transport of one cited bound.
//!
//! The one- and two-alias walks are intentionally separate sources. Neither
//! takes a hop count or searches an alias graph: each follows exactly the
//! number of distinct cited equalities its shape names.

use std::cell::RefCell;
use std::rc::Rc;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::integer_evidence::{
    Citation, cited_facts, closed_integer_relation, literal_value,
};
use super::super::scope_identity::{self, ScopeDerived};
use super::literal::goal_literal_at;
use super::roster::{
    EndpointBounds, EndpointEqualities, distinct_same_carrier_subjects, ordered_at,
    oriented_equality, substitute_subject,
};
use super::{Complete, Request};

/// A cited bound on `alias` renamed to `root` through one cited equality
/// `root == alias`.
pub(super) fn one_alias(request: Request, complete: &mut dyn Complete) -> Option<ProofNode> {
    let bounds_by_endpoint = EndpointBounds::new(request);
    for &(citation, reversed) in bounded_alias_orientations(request, &bounds_by_endpoint).iter() {
        let (equality_citation, equality, root, alias) =
            oriented_equality(request, citation, reversed);
        if !complete.admits(root) {
            continue;
        }
        for (relation_citation, relation, bound, endpoint) in
            bounds_by_endpoint.value_candidates(alias)
        {
            let root_bound = substitute_subject(
                relation_citation.proof(relation),
                bound,
                endpoint,
                [(root, equality_citation.proof(equality))],
            );
            if let Some(proof) = complete.complete(root, root_bound) {
                return Some(proof);
            }
        }
    }
    None
}

/// The `root == alias` readings of cited equalities, in citation order and
/// left reading first, between distinct same-carrier subjects whose alias
/// carries a cited value bound: the only readings [`one_alias`] can build a
/// candidate from. Every goal under one scope walks the same readings, so
/// they are derived once per scope rather than rescanned per goal.
fn bounded_alias_orientations(
    request: Request,
    bounds_by_endpoint: &EndpointBounds,
) -> Rc<Vec<(Citation, bool)>> {
    thread_local! {
        static ORIENTATIONS: RefCell<ScopeDerived<Vec<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    scope_identity::derived(
        &ORIENTATIONS,
        request.assumptions,
        request.semantic_axioms,
        || {
            equality_readings(request)
                .filter(|&(_, _, root, alias)| {
                    distinct_same_carrier_subjects(root, alias)
                        && bounds_by_endpoint.value_candidates(alias).next().is_some()
                })
                .map(|(citation, reversed, _, _)| (citation, reversed))
                .collect()
        },
    )
}

/// A cited bound on `bound_alias` renamed to `middle_alias` and then to `root`
/// through two distinct cited equalities `middle_alias == bound_alias` and
/// `root == middle_alias`.
pub(super) fn two_alias(request: Request, complete: &mut dyn Complete) -> Option<ProofNode> {
    let bounds_by_endpoint = EndpointBounds::new(request);
    let aliases_by_endpoint = EndpointEqualities::new(request);
    for &(outer_citation, reversed) in
        two_alias_orientations(request, &bounds_by_endpoint, &aliases_by_endpoint).iter()
    {
        let (outer_citation, outer_equality, root, middle_alias) =
            oriented_equality(request, outer_citation, reversed);
        // Every inner alias and bound below shares this root, so a root the
        // completion refuses skips the whole two-hop enumeration.
        if !complete.admits(root) {
            continue;
        }
        for (inner_citation, inner_equality, bound_alias) in
            aliases_by_endpoint.candidates(middle_alias)
        {
            if std::ptr::eq(outer_equality, inner_equality) {
                continue;
            }
            if bound_alias == root || !distinct_same_carrier_subjects(middle_alias, bound_alias) {
                continue;
            }
            for (relation_citation, relation, bound, endpoint) in
                bounds_by_endpoint.value_candidates(bound_alias)
            {
                let root_bound = substitute_subject(
                    relation_citation.proof(relation),
                    bound,
                    endpoint,
                    [
                        (middle_alias, inner_citation.proof(inner_equality)),
                        (root, outer_citation.proof(outer_equality)),
                    ],
                );
                if let Some(proof) = complete.complete(root, root_bound) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

/// The `root == middle` readings [`two_alias`] can build a candidate from:
/// distinct same-carrier subjects whose middle has some cited alias, distinct
/// from it, that carries a cited value bound. The per-reading filters that
/// depend on the outer equality itself stay in the walk; this keeps exactly
/// the readings that pass them for some outer equality, in walk order, once
/// per scope.
fn two_alias_orientations(
    request: Request,
    bounds_by_endpoint: &EndpointBounds,
    aliases_by_endpoint: &EndpointEqualities,
) -> Rc<Vec<(Citation, bool)>> {
    thread_local! {
        static ORIENTATIONS: RefCell<ScopeDerived<Vec<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    scope_identity::derived(
        &ORIENTATIONS,
        request.assumptions,
        request.semantic_axioms,
        || {
            equality_readings(request)
                .filter(|&(_, _, root, middle)| {
                    distinct_same_carrier_subjects(root, middle)
                        && aliases_by_endpoint
                            .candidates(middle)
                            .any(|(_, _, bound_alias)| {
                                distinct_same_carrier_subjects(middle, bound_alias)
                                    && bounds_by_endpoint
                                        .value_candidates(bound_alias)
                                        .next()
                                        .is_some()
                            })
                })
                .map(|(citation, reversed, _, _)| (citation, reversed))
                .collect()
        },
    )
}

/// Every cited equality read `left == right`, then `right == left`, as
/// `(citation, reversed, root, alias)`.
fn equality_readings<'a>(
    request: Request<'a>,
) -> impl Iterator<Item = (Citation, bool, &'a ScalarTerm, &'a ScalarTerm)> {
    cited_facts(request.assumptions, request.semantic_axioms).flat_map(|(citation, fact)| {
        let readings = match fact {
            Proposition::Equal(left, right) => {
                vec![
                    (citation, false, left, right),
                    (citation, true, right, left),
                ]
            }
            _ => Vec::new(),
        };
        readings.into_iter()
    })
}

/// A cited `alias` bound whose literal is at least as strong as the goal's
/// literal side, closed against it and renamed to `root` through one cited
/// equality `root == alias`.
pub(super) fn stronger_alias_bound(
    request: Request,
    complete: &mut dyn Complete,
) -> Option<ProofNode> {
    let relations_by_endpoint = EndpointBounds::new(request);
    for (equality_citation, equality) in cited_facts(request.assumptions, request.semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        'root: for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if !distinct_same_carrier_subjects(root, alias) {
                continue;
            }
            let mut admitted = None;
            for (bound_citation, bound_fact, retained_literal, endpoint) in
                relations_by_endpoint.candidates(alias)
            {
                let Some((integer_type, _)) = literal_value(retained_literal) else {
                    continue;
                };
                if root.scalar_type() != ScalarType::Integer(integer_type) {
                    continue;
                }
                let Some(goal_literal) = goal_literal_at(request.goal, root, endpoint) else {
                    continue;
                };
                let Some(closed_bridge) =
                    closed_integer_relation(ordered_at(retained_literal, &goal_literal, endpoint))
                else {
                    continue;
                };
                if !*admitted.get_or_insert_with(|| complete.admits(root)) {
                    continue 'root;
                }
                let retained_bound = bound_citation.proof(bound_fact);
                let (left_less_or_equal_middle, middle_less_or_equal_right) = if endpoint == 1 {
                    (closed_bridge, retained_bound)
                } else {
                    (retained_bound, closed_bridge)
                };
                let alias_bound = ProofNode {
                    conclusion: ordered_at(alias, &goal_literal, endpoint),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
                        middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
                    },
                };
                let root_bound = substitute_subject(
                    alias_bound,
                    &goal_literal,
                    endpoint,
                    [(root, equality_citation.proof(equality))],
                );
                if let Some(proof) = complete.complete(root, root_bound) {
                    return Some(proof);
                }
            }
        }
    }
    None
}
