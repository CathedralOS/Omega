//! Fixed-depth value-alias transport for canonical order certificates.
//!
//! The one- and two-alias shapes are intentionally separate entry points. This
//! module exposes neither a hop-count parameter nor recursive graph search.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::integer_evidence::{Citation, cited_facts};

mod cast;

pub(super) use cast::{prove_landed_literal_cast, prove_stronger_cast};

pub(super) fn prove_one(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let bounds_by_endpoint = indexed_bounds(assumptions, semantic_axioms);
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, alias) in [(left, right), (right, left)] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            let Some(bounds) = bounds_by_endpoint.get(alias) else {
                continue;
            };
            for &(relation_citation, relation, endpoint) in bounds {
                let root_bound = substitute_bound_endpoint(relation, root, endpoint);
                let proof = ProofNode {
                    conclusion: root_bound,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation_citation.proof(relation)),
                        equality: Box::new(equality_citation.proof(equality)),
                        endpoint,
                    },
                };
                if let Some(proof) = complete(root, proof) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

pub(super) fn prove_two(
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let bounds_by_endpoint = indexed_bounds(assumptions, semantic_axioms);
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (root, middle_alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            if !distinct_same_carrier_values(root, middle_alias) {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let bound_alias = if inner_left == middle_alias {
                    inner_right
                } else if inner_right == middle_alias {
                    inner_left
                } else {
                    continue;
                };
                if bound_alias == root || !distinct_same_carrier_values(middle_alias, bound_alias) {
                    continue;
                }
                let Some(bounds) = bounds_by_endpoint.get(bound_alias) else {
                    continue;
                };
                for &(relation_citation, relation, endpoint) in bounds {
                    let middle_bound = ProofNode {
                        conclusion: substitute_bound_endpoint(relation, middle_alias, endpoint),
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(relation_citation.proof(relation)),
                            equality: Box::new(inner_citation.proof(inner_equality)),
                            endpoint,
                        },
                    };
                    let root_bound = ProofNode {
                        conclusion: substitute_bound_endpoint(relation, root, endpoint),
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(middle_bound),
                            equality: Box::new(outer_citation.proof(outer_equality)),
                            endpoint,
                        },
                    };
                    if let Some(proof) = complete(root, root_bound) {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}

fn indexed_bounds<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, usize)>> {
    let mut bounds_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, fact, 0));
        }
        if right != left && matches!(right, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((citation, fact, 1));
        }
    }
    bounds_by_endpoint
}

pub(super) fn distinct_same_carrier_values(left: &ScalarTerm, right: &ScalarTerm) -> bool {
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
