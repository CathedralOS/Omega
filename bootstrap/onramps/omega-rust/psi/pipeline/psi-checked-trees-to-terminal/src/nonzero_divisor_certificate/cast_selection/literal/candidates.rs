//! Source-ordered landed-literal cast candidates for production.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

use super::super::super::integer_evidence::cited_facts;
use super::completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, literal) in [(left, right), (right, left)] {
            if !matches!(root, ScalarTerm::Value { .. }) {
                continue;
            }
            let Some((integer_type, _)) = literal.integer_value() else {
                continue;
            };
            if root.scalar_type() != ScalarType::Integer(integer_type) {
                continue;
            }
            if let Some(proof) = completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                literal,
                citation.proof(equality),
            ) {
                return Some(proof);
            }
        }
    }
    None
}
