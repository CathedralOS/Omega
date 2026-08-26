//! Source-ordered direct cast root-bound candidates for production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::super::integer_evidence::cited_facts;
use super::completion;

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
        if let Some(proof) = completion::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root_left,
            root_right,
            citation.proof(root_bound),
        ) {
            return Some(proof);
        }
    }
    None
}
