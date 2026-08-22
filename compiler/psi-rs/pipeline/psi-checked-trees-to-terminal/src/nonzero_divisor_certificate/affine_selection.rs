//! Side-local selection of retained evidence for bounded affine proofs.

use std::collections::BTreeMap;

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::integer_evidence::cited_facts;
use super::{affine_custody, alias_transport};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = affine_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                citation.proof(root_bound),
            ) {
                return Some(proof);
            }
        }
    }
    prove_landed_literal_affine_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| {
            prove_alias_substituted_affine_bound(context, goal, assumptions, semantic_axioms)
        })
        .or_else(|| {
            prove_transitively_reconstructed_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| {
            prove_transitively_alias_substituted_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| {
            prove_two_alias_substituted_affine_bound(context, goal, assumptions, semantic_axioms)
        })
}

fn prove_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, literal) in [(left, right), (right, left)] {
            if !matches!(root, psi_core::ScalarTerm::Value { .. }) {
                continue;
            }
            let Some((integer_type, _)) = literal.integer_value() else {
                continue;
            };
            if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                continue;
            }
            let reflexive = Proposition::LessOrEqual(literal.clone(), literal.clone());
            for (root_bound, endpoint) in [
                (Proposition::LessOrEqual(literal.clone(), root.clone()), 1),
                (Proposition::LessOrEqual(root.clone(), literal.clone()), 0),
            ] {
                let root_bound = ProofNode {
                    conclusion: root_bound,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: reflexive.clone(),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        equality: Box::new(citation.proof(equality)),
                        endpoint,
                    },
                };
                if let Some(proof) = affine_custody::prove_from_root(
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
        }
    }

    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            if root == alias
                || !matches!(root, psi_core::ScalarTerm::Value { .. })
                || !matches!(alias, psi_core::ScalarTerm::Value { .. })
                || root.scalar_type() != alias.scalar_type()
            {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let literal = if inner_left == alias {
                    inner_right
                } else if inner_right == alias {
                    inner_left
                } else {
                    continue;
                };
                let Some((integer_type, _)) = literal.integer_value() else {
                    continue;
                };
                if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                    continue;
                }
                let reflexive = Proposition::LessOrEqual(literal.clone(), literal.clone());
                for (alias_bound, root_bound, endpoint) in [
                    (
                        Proposition::LessOrEqual(literal.clone(), alias.clone()),
                        Proposition::LessOrEqual(literal.clone(), root.clone()),
                        1,
                    ),
                    (
                        Proposition::LessOrEqual(alias.clone(), literal.clone()),
                        Proposition::LessOrEqual(root.clone(), literal.clone()),
                        0,
                    ),
                ] {
                    let alias_bound = ProofNode {
                        conclusion: alias_bound,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: reflexive.clone(),
                                rule: ProofRule::Primitive(
                                    PrimitiveJudgment::ClosedIntegerRelation,
                                ),
                            }),
                            equality: Box::new(inner_citation.proof(inner_equality)),
                            endpoint,
                        },
                    };
                    let root_bound = ProofNode {
                        conclusion: root_bound,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(alias_bound),
                            equality: Box::new(outer_citation.proof(outer_equality)),
                            endpoint,
                        },
                    };
                    if let Some(proof) = affine_custody::prove_from_root(
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
            }
        }
    }
    None
}

fn prove_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
        affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}

/// Transport one exact retained bound through exactly two distinct value
/// equalities before constructing the affine proof. The equality walk is fixed
/// at depth two; this does not recurse or enumerate a general alias graph.
fn prove_two_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_two(assumptions, semantic_axioms, |root, root_bound| {
        affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}

/// Reconstruct one affine-root bound through exactly two ordered citations and
/// one exact value equality. This deliberately calls the affine constructor
/// directly: it does not recurse through the general integer-bound search, so
/// neither equality chains nor longer order paths are admitted here.
fn prove_transitively_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, _) = fact else {
            continue;
        };
        if matches!(left, psi_core::ScalarTerm::Value { .. }) {
            bounds_by_left_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, fact));
        }
    }

    for (equality_citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = equality else {
            continue;
        };
        for (root, alias) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if root == alias
                || !matches!(root, psi_core::ScalarTerm::Value { .. })
                || !matches!(alias, psi_core::ScalarTerm::Value { .. })
            {
                continue;
            }
            for (left_citation, left_fact) in cited_facts(assumptions, semantic_axioms) {
                let Proposition::LessOrEqual(left, middle) = left_fact else {
                    continue;
                };
                if !matches!(middle, psi_core::ScalarTerm::Value { .. }) {
                    continue;
                }
                let Some(right_facts) = bounds_by_left_endpoint.get(middle) else {
                    continue;
                };
                for &(right_citation, right_fact) in right_facts {
                    if std::ptr::eq(left_fact, right_fact) {
                        continue;
                    }
                    let Proposition::LessOrEqual(_, right) = right_fact else {
                        unreachable!("only integer bounds are indexed")
                    };
                    let (endpoint, conclusion) = if alias == left {
                        (0, Proposition::LessOrEqual(root.clone(), right.clone()))
                    } else if alias == right {
                        (1, Proposition::LessOrEqual(left.clone(), root.clone()))
                    } else {
                        continue;
                    };
                    let transitive = ProofNode {
                        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                        rule: ProofRule::IntegerLessOrEqualTransitivity {
                            left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                            middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                        },
                    };
                    let root_bound = ProofNode {
                        conclusion,
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
                            relation: Box::new(transitive),
                            equality: Box::new(equality_citation.proof(equality)),
                            endpoint,
                        },
                    };
                    if let Some(proof) = affine_custody::prove_from_root(
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
            }
        }
    }
    None
}

fn prove_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, _) = fact else {
            continue;
        };
        if matches!(left, psi_core::ScalarTerm::Value { .. }) {
            bounds_by_left_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, fact));
        }
    }

    for (left_citation, left_fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            continue;
        };
        if !matches!(middle, psi_core::ScalarTerm::Value { .. }) {
            continue;
        }
        let Some(right_facts) = bounds_by_left_endpoint.get(middle) else {
            continue;
        };
        for &(right_citation, right_fact) in right_facts {
            if std::ptr::eq(left_fact, right_fact) {
                continue;
            }
            let Proposition::LessOrEqual(_, right) = right_fact else {
                unreachable!("only integer bounds are indexed")
            };
            let conclusion = Proposition::LessOrEqual(left.clone(), right.clone());
            let root_bound = ProofNode {
                conclusion,
                rule: ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                    middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                },
            };
            for root in [left, right]
                .into_iter()
                .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
            {
                if let Some(proof) = affine_custody::prove_from_root(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    root,
                    root_bound.clone(),
                ) {
                    return Some(proof);
                }
            }
        }
    }
    None
}
