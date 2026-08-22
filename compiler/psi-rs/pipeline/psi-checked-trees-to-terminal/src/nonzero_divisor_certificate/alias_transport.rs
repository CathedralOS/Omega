//! Fixed-depth value-alias transport for canonical order certificates.
//!
//! The one- and two-alias shapes are intentionally separate entry points. This
//! module exposes neither a hop-count parameter nor recursive graph search.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::{
    cited_facts, closed_integer_relation, prove_cast_bound_from_root, remap_integer_literal,
};

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

pub(super) fn prove_stronger_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            for (bound_citation, bound) in cited_facts(assumptions, semantic_axioms) {
                let Proposition::LessOrEqual(bound_left, bound_right) = bound else {
                    continue;
                };
                let (literal, endpoint) = if bound_left == alias {
                    (bound_right, 0)
                } else if bound_right == alias {
                    (bound_left, 1)
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
                if let Some(proof) = prove_cast_from_stronger_bound(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    alias,
                    literal,
                    endpoint,
                    bound_citation.proof(bound),
                    equality_citation.proof(equality),
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn prove_cast_from_stronger_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    alias: &ScalarTerm,
    retained_literal: &ScalarTerm,
    endpoint: usize,
    retained_bound: ProofNode,
    equality: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return None;
    };
    let (target, target_endpoint) = if endpoint == 1 {
        (goal_right, goal_left)
    } else {
        (goal_left, goal_right)
    };
    if !matches!(target, ScalarTerm::Value { .. }) {
        return None;
    }
    let source_endpoint = remap_integer_literal(target_endpoint, root_type)?;
    let closed_bridge = if endpoint == 1 {
        closed_integer_relation(Proposition::LessOrEqual(
            source_endpoint.clone(),
            retained_literal.clone(),
        ))?
    } else {
        closed_integer_relation(Proposition::LessOrEqual(
            retained_literal.clone(),
            source_endpoint.clone(),
        ))?
    };
    let alias_bound = ProofNode {
        conclusion: if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint.clone(), alias.clone())
        } else {
            Proposition::LessOrEqual(alias.clone(), source_endpoint.clone())
        },
        rule: if endpoint == 1 {
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(closed_bridge),
                middle_less_or_equal_right: Box::new(retained_bound),
            }
        } else {
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(retained_bound),
                middle_less_or_equal_right: Box::new(closed_bridge),
            }
        },
    };
    let root_bound = ProofNode {
        conclusion: if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint, root.clone())
        } else {
            Proposition::LessOrEqual(root.clone(), source_endpoint)
        },
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(alias_bound),
            equality: Box::new(equality),
            endpoint,
        },
    };
    prove_cast_bound_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        root,
        root_bound,
    )
}

pub(super) fn prove_landed_literal_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (root_citation, root_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(root_left, root_right) = root_equality else {
            continue;
        };
        for (root, alias) in [(root_left, root_right), (root_right, root_left)] {
            if !distinct_same_carrier_values(root, alias) {
                continue;
            }
            for (literal_citation, literal_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(root_equality, literal_equality) {
                    continue;
                }
                let Proposition::Equal(literal_left, literal_right) = literal_equality else {
                    continue;
                };
                let literal = if literal_left == alias {
                    literal_right
                } else if literal_right == alias {
                    literal_left
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
                if let Some(proof) = prove_cast_from_landed_literal(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    alias,
                    literal,
                    root_citation.proof(root_equality),
                    literal_citation.proof(literal_equality),
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn prove_cast_from_landed_literal(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    alias: &ScalarTerm,
    landed_literal: &ScalarTerm,
    root_equality: ProofNode,
    literal_equality: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return None;
    };
    for (target, target_endpoint, endpoint) in
        [(goal_right, goal_left, 1), (goal_left, goal_right, 0)]
    {
        if !matches!(target, ScalarTerm::Value { .. }) {
            continue;
        }
        let Some(source_endpoint) = remap_integer_literal(target_endpoint, root_type) else {
            continue;
        };
        let closed_relation = if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint.clone(), landed_literal.clone())
        } else {
            Proposition::LessOrEqual(landed_literal.clone(), source_endpoint.clone())
        };
        let Some(closed_relation) = closed_integer_relation(closed_relation) else {
            continue;
        };
        let alias_bound = ProofNode {
            conclusion: if endpoint == 1 {
                Proposition::LessOrEqual(source_endpoint.clone(), alias.clone())
            } else {
                Proposition::LessOrEqual(alias.clone(), source_endpoint.clone())
            },
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(closed_relation),
                equality: Box::new(literal_equality.clone()),
                endpoint,
            },
        };
        let root_bound = ProofNode {
            conclusion: if endpoint == 1 {
                Proposition::LessOrEqual(source_endpoint, root.clone())
            } else {
                Proposition::LessOrEqual(root.clone(), source_endpoint)
            },
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(alias_bound),
                equality: Box::new(root_equality.clone()),
                endpoint,
            },
        };
        if let Some(proof) = prove_cast_bound_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}

fn indexed_bounds<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(super::Citation, &'a Proposition, usize)>> {
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
