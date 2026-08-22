//! Direct two-citation affine completion for production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::affine_custody::{self, DefinitionIndex};

mod bound;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    left: &ScalarTerm,
    right: &ScalarTerm,
    left_proof: ProofNode,
    right_proof: ProofNode,
) -> Option<ProofNode> {
    let root_bound = bound::prove(left, right, left_proof, right_proof);
    for root in [left, right]
        .into_iter()
        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
    {
        if let Some(proof) = affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            root,
            root_bound.clone(),
        ) {
            return Some(proof);
        }
    }
    None
}
