//! Direct retained affine-root bound proof selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;
use super::super::integer_evidence::cited_facts;

mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                root_bound,
                citation,
            ) {
                return Some(proof);
            }
        }
    }
    None
}
