//! Source-ordered affine root-alias equalities for certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct RootAliases<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> RootAliases<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn find<T>(
        &self,
        mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> Option<T>,
    ) -> Option<T> {
        for (citation, equality) in cited_facts(self.assumptions, self.semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (root, alias) in [(left, right), (right, left)] {
                if root == alias
                    || !matches!(root, ScalarTerm::Value { .. })
                    || !matches!(alias, ScalarTerm::Value { .. })
                    || root.scalar_type() != alias.scalar_type()
                {
                    continue;
                }
                if let Some(result) = join(citation, equality, root, alias) {
                    return Some(result);
                }
            }
        }
        None
    }
}
