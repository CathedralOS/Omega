//! Typed stronger alias-bound completion for exact integer casts.

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
