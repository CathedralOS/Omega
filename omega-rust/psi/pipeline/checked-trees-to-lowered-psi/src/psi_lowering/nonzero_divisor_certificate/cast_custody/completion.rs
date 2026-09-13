//! Exact integer-cast witness and certificate completion for production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

mod target;

pub(in super::super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        if let Some(proof) = target::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            &root_bound,
            target,
        ) {
            return Some(proof);
        }
    }
    None
}
