//! Source-ordered alias-to-literal landing index for independent reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

pub(super) struct LandingIndex<'a> {
    by_alias: BTreeMap<ScalarTerm, Vec<(&'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = BTreeMap::<_, Vec<_>>::new();
        for equality in requirements.iter().chain(semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (alias, literal) in [(left, right), (right, left)] {
                if matches!(alias, ScalarTerm::Value { .. }) && literal.integer_value().is_some() {
                    by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push((equality, literal));
                }
            }
        }
        Self { by_alias }
    }

    pub(super) fn candidates(&self, alias: &ScalarTerm) -> &[(&'a Proposition, &'a ScalarTerm)] {
        self.by_alias.get(alias).map(Vec::as_slice).unwrap_or(&[])
    }
}
