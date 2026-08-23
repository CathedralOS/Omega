//! Source-ordered alias-to-literal landing index for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::{equalities, fact_identity, value_index::ValueIndex};

pub(super) struct LandingIndex<'a> {
    by_alias: ValueIndex<(&'a Proposition, &'a ScalarTerm)>,
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut by_alias = ValueIndex::new();
        equalities::exact_value_bindings(requirements, semantic_axioms).for_each(
            |(equality, alias, literal)| {
                by_alias.push(alias, (equality, literal));
            },
        );
        Self { by_alias }
    }

    pub(super) fn any(
        &self,
        root: &ScalarTerm,
        alias: &ScalarTerm,
        outer_equality: &Proposition,
        mut complete: impl FnMut(&'a ScalarTerm) -> bool,
    ) -> bool {
        if root.scalar_type() != alias.scalar_type() {
            return false;
        }
        self.by_alias
            .candidates(alias)
            .iter()
            .any(|&(inner_equality, literal)| {
                fact_identity::distinct(outer_equality, inner_equality) && complete(literal)
            })
    }
}
