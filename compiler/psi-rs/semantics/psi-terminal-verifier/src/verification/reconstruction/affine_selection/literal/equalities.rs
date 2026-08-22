//! Source-ordered oriented equalities for independent affine-literal reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct OrientedEqualities<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> OrientedEqualities<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
        }
    }

    pub(super) fn iter(
        &self,
    ) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + '_ {
        self.requirements
            .iter()
            .chain(self.semantic_axioms)
            .filter_map(|equality| match equality {
                Proposition::Equal(left, right) => Some((equality, left, right)),
                _ => None,
            })
            .flat_map(|(equality, left, right)| [(equality, left, right), (equality, right, left)])
    }
}
