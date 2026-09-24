//! Typed stronger alias-bound completion for exact integer casts.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::cast_custody;

mod bound;

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
    let semantic_vocabulary::ScalarType::Integer(root_type) = root.scalar_type() else {
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
    let root_bound = bound::prove(
        root,
        alias,
        retained_literal,
        source_endpoint,
        endpoint,
        retained_bound,
        equality,
    )?;
    cast_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        root,
        root_bound,
    )
}
