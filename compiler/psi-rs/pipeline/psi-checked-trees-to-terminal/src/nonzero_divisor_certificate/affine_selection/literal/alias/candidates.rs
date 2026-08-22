//! Source-ordered one-alias literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::cited_facts;

mod landing_index;

use landing_index::LandingIndex;

pub(super) struct LiteralAliasCandidates<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    landings: LandingIndex<'a>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
            landings: LandingIndex::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(
            &'a ScalarTerm,
            &'a ScalarTerm,
            &'a ScalarTerm,
            ProofNode,
            ProofNode,
        ) -> Option<T>,
    ) -> Option<T> {
        for (outer_citation, outer_equality) in cited_facts(self.assumptions, self.semantic_axioms)
        {
            let Proposition::Equal(outer_left, outer_right) = outer_equality else {
                continue;
            };
            for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
                if root == alias
                    || !matches!(root, ScalarTerm::Value { .. })
                    || !matches!(alias, ScalarTerm::Value { .. })
                    || root.scalar_type() != alias.scalar_type()
                {
                    continue;
                }
                for &(inner_citation, inner_equality, literal) in self.landings.candidates(alias) {
                    if std::ptr::eq(outer_equality, inner_equality) {
                        continue;
                    }
                    let Some((integer_type, _)) = literal.integer_value() else {
                        unreachable!("literal index contains only integer landings")
                    };
                    if root.scalar_type() != ScalarType::Integer(integer_type) {
                        continue;
                    }
                    if let Some(result) = complete(
                        root,
                        alias,
                        literal,
                        outer_citation.proof(outer_equality),
                        inner_citation.proof(inner_equality),
                    ) {
                        return Some(result);
                    }
                }
            }
        }
        None
    }
}
