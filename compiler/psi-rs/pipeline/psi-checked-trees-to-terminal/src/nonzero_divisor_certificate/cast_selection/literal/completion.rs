//! Typed direct landed-literal completion for exact integer casts.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::cast_custody;

mod bound;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
    equality: ProofNode,
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
        let Some(root_bound) = bound::prove(
            root,
            landed_literal,
            source_endpoint,
            endpoint,
            equality.clone(),
        ) else {
            continue;
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
