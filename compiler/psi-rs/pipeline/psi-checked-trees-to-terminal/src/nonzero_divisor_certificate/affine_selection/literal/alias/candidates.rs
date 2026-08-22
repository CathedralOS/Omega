//! Source-ordered one-alias literal candidates for certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

use super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct LiteralAliasCandidates<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    literals_by_alias: BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut literals_by_alias = BTreeMap::<_, Vec<_>>::new();
        for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (alias, literal) in [(left, right), (right, left)] {
                if matches!(alias, ScalarTerm::Value { .. }) && literal.integer_value().is_some() {
                    literals_by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push((citation, equality, literal));
                }
            }
        }
        Self {
            assumptions,
            semantic_axioms,
            literals_by_alias,
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
                let Some(landings) = self.literals_by_alias.get(alias) else {
                    continue;
                };
                for &(inner_citation, inner_equality, literal) in landings {
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
