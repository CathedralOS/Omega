//! Direct retained root-bound completion for exact integer casts.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::cast_custody;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root_left: &ScalarTerm,
    root_right: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    for root in [root_left, root_right]
        .into_iter()
        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
    {
        if let Some(proof) = cast_custody::prove_from_root(
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
    None
}
