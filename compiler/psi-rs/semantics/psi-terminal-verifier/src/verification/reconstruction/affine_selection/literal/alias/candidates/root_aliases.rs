//! Source-ordered affine root-alias equalities for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct RootAliases<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> RootAliases<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
        }
    }

    pub(super) fn any(
        &self,
        mut join: impl FnMut(&'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        for equality in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (root, alias) in [(left, right), (right, left)] {
                if root != alias
                    && matches!(root, ScalarTerm::Value { .. })
                    && matches!(alias, ScalarTerm::Value { .. })
                    && root.scalar_type() == alias.scalar_type()
                    && join(equality, root, alias)
                {
                    return true;
                }
            }
        }
        false
    }
}
