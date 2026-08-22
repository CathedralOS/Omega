//! Typed landed-literal alias completion for exact integer casts.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::super::cast_custody;
use super::super::super::super::integer_evidence::closed_integer_relation;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
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
