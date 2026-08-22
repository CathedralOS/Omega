//! Source-ordered alias-to-literal landing index for independent reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::{eligibility, equalities};

pub(super) struct LandingIndex<'a> {
    by_alias: BTreeMap<ScalarTerm, Vec<(&'a Proposition, &'a ScalarTerm)>>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = BTreeMap::<_, Vec<_>>::new();
        equalities::ordered(requirements, semantic_axioms).for_each(
            |(equality, alias, literal)| {
                if eligibility::is_value(alias)
                    && eligibility::integer_literal_type(literal).is_some()
                {
                    by_alias
                        .entry(alias.clone())
                        .or_default()
                        .push((equality, literal));
                }
            },
        );
        Self { by_alias }
    }

    pub(super) fn candidates(&self, alias: &ScalarTerm) -> &[(&'a Proposition, &'a ScalarTerm)] {
        self.by_alias.get(alias).map(Vec::as_slice).unwrap_or(&[])
    }
}
