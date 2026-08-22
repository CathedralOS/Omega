//! Alias-landed literals for exact integer-cast completion.

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
        let Some(source_endpoint) = cast_custody::remap_integer_literal(target_endpoint, root_type)
        else {
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
        if let Some(proof) = cast_custody::prove_from_root(
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
