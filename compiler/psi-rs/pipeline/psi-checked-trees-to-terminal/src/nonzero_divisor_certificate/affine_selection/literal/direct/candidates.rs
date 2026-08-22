//! Source-ordered direct landed-literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::cited_facts;

pub(super) struct DirectLiteralCandidates<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> DirectLiteralCandidates<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode) -> Option<T>,
    ) -> Option<T> {
        for (citation, equality) in cited_facts(self.assumptions, self.semantic_axioms) {
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
                if let Some(result) = complete(root, literal, citation.proof(equality)) {
                    return Some(result);
                }
            }
        }
        None
    }
}
