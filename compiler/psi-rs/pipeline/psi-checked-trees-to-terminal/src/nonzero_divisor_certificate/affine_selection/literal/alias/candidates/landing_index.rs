//! Source-ordered alias-to-literal landing index for certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct LandingIndex<'a> {
    by_alias: BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = BTreeMap::<_, Vec<_>>::new();
        for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (alias, literal) in [(left, right), (right, left)] {
                if matches!(alias, ScalarTerm::Value { .. }) && literal.integer_value().is_some() {
                    by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push((citation, equality, literal));
                }
            }
        }
        Self { by_alias }
    }

    pub(super) fn candidates(
        &self,
        alias: &ScalarTerm,
    ) -> &[(Citation, &'a Proposition, &'a ScalarTerm)] {
        self.by_alias.get(alias).map(Vec::as_slice).unwrap_or(&[])
    }
}
