//! Source-ordered direct landed-literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::{eligibility, equalities::OrientedEqualities};

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
        self.equalities
            .iter()
            .find_map(|(citation, equality, root, literal)| {
                if !eligibility::exact_value_binding(root, literal) {
                    return None;
                }
                complete(root, literal, citation.proof(equality))
            })
    }
}
