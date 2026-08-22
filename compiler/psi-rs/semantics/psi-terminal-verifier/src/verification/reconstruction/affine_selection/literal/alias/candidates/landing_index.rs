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
        equalities::exact_value_bindings(requirements, semantic_axioms).for_each(
            |(equality, alias, literal)| {
                by_alias
                    .entry(alias.clone())
                    .or_default()
                    .push((equality, literal));
            },
        );
        Self { by_alias }
    }

    pub(super) fn any(
        &self,
        alias: &ScalarTerm,
        outer_equality: &Proposition,
        mut complete: impl FnMut(&'a ScalarTerm) -> bool,
    ) -> bool {
        self.by_alias
            .get(alias)
            .into_iter()
            .flatten()
            .any(|&(inner_equality, literal)| {
                eligibility::distinct_facts(outer_equality, inner_equality) && complete(literal)
            })
    }
}
