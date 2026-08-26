//! Source-ordered alias-to-literal landing index for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_admission::ProofNode;

use super::super::super::super::super::integer_evidence::Citation;
use super::super::super::super::{equalities, fact_identity, value_index::ValueIndex};

pub(super) struct LandingIndex<'a> {
    by_alias: ValueIndex<(Citation, &'a Proposition, &'a ScalarTerm)>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = ValueIndex::new();
        equalities::exact_value_bindings(assumptions, semantic_axioms).for_each(
            |(citation, equality, alias, literal)| {
                by_alias.push(alias, (citation, equality, literal));
            },
        );
        Self { by_alias }
    }

    pub(super) fn find<T>(
        &self,
        root: &ScalarTerm,
        alias: &ScalarTerm,
        outer_citation: Citation,
        outer_equality: &Proposition,
        mut complete: impl FnMut(&'a ScalarTerm, ProofNode, ProofNode) -> Option<T>,
    ) -> Option<T> {
        if root.scalar_type() != alias.scalar_type() {
            return None;
        }
        self.by_alias
            .candidates(alias)
            .iter()
            .find_map(|&(citation, inner_equality, literal)| {
                if !fact_identity::distinct(outer_equality, inner_equality) {
                    return None;
                }
                complete(
                    literal,
                    outer_citation.proof(outer_equality),
                    citation.proof(inner_equality),
                )
            })
    }
}
