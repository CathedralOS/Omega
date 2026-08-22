//! Source-ordered alias-to-literal landing index for certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::super::super::integer_evidence::Citation;
use super::super::super::super::{eligibility, equalities};

pub(super) struct LandingIndex<'a> {
    by_alias: BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = BTreeMap::<_, Vec<_>>::new();
        equalities::exact_value_bindings(assumptions, semantic_axioms).for_each(
            |(citation, equality, alias, literal)| {
                by_alias
                    .entry(alias.clone())
                    .or_default()
                    .push((citation, equality, literal));
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
        for &(citation, inner_equality, literal) in self.by_alias.get(alias).into_iter().flatten() {
            if !eligibility::distinct_facts(outer_equality, inner_equality) {
                continue;
            }
            if let Some(result) = complete(
                literal,
                outer_citation.proof(outer_equality),
                citation.proof(inner_equality),
            ) {
                return Some(result);
            }
        }
        None
    }
}
