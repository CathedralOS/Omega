//! Source-ordered affine root-alias equalities for certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::Citation;
use super::super::super::equalities::OrientedEqualities;

pub(super) struct RootAliases<'a> {
    equalities: OrientedEqualities<'a>,
}

impl<'a> RootAliases<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            equalities: OrientedEqualities::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> Option<T>,
    ) -> Option<T> {
        self.equalities.find(|citation, equality, root, alias| {
            if root == alias
                || !matches!(root, ScalarTerm::Value { .. })
                || !matches!(alias, ScalarTerm::Value { .. })
                || root.scalar_type() != alias.scalar_type()
            {
                return None;
            }
            join(citation, equality, root, alias)
        })
    }
}
