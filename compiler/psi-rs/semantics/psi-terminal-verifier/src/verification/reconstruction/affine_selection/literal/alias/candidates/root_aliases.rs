//! Source-ordered affine root-alias equalities for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::equalities::OrientedEqualities;

pub(super) struct RootAliases<'a> {
    equalities: OrientedEqualities<'a>,
}

impl<'a> RootAliases<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            equalities: OrientedEqualities::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut join: impl FnMut(&'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.equalities.any(|equality, root, alias| {
            root != alias
                && matches!(root, ScalarTerm::Value { .. })
                && matches!(alias, ScalarTerm::Value { .. })
                && root.scalar_type() == alias.scalar_type()
                && join(equality, root, alias)
        })
    }
}
