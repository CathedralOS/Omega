//! Closed-strengthened alias bounds for exact integer-cast completion.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::cast_custody;
use super::super::super::integer_evidence::{cited_facts, closed_integer_relation};
use super::super::distinct_same_carrier_values;

pub(super) fn prove(
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
    let source_endpoint = cast_custody::remap_integer_literal(target_endpoint, root_type)?;
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
    cast_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        root,
        root_bound,
    )
}
