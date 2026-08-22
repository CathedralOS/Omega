//! Affine-root custody for one following exact partial-cast spine.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    for (target, literal, target_is_right) in [(right, left, true), (left, right, false)] {
        if !matches!(target, ScalarTerm::Value { .. }) {
            continue;
        }
        if let Some(proof) = completion::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            target,
            literal,
            target_is_right,
        ) {
            return Some(proof);
        }
    }
    None
}
