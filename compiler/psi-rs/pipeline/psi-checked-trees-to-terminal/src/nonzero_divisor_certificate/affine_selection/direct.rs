//! Direct retained affine-root bound proof selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody;
use super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = affine_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                citation.proof(root_bound),
            ) {
                return Some(proof);
            }
        }
    }
    None
}
