//! Source-ordered direct landed-literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

mod equalities;

use equalities::OrientedEqualities;

pub(super) struct DirectLiteralCandidates<'a> {
    equalities: OrientedEqualities<'a>,
}

impl<'a> DirectLiteralCandidates<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            equalities: OrientedEqualities::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode) -> Option<T>,
    ) -> Option<T> {
        self.equalities.find(|citation, equality, root, literal| {
            if !matches!(root, ScalarTerm::Value { .. }) {
                return None;
            }
            let Some((integer_type, _)) = literal.integer_value() else {
                return None;
            };
            if root.scalar_type() != ScalarType::Integer(integer_type) {
                return None;
            }
            complete(root, literal, citation.proof(equality))
        })
    }
}
