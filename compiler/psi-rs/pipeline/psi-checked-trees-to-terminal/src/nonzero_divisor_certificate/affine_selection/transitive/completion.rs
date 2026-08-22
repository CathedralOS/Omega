//! Direct two-citation affine completion for production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::affine_custody::{self, DefinitionIndex};

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
    let root_bound = ProofNode {
        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_proof),
            middle_less_or_equal_right: Box::new(right_proof),
        },
    };
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
