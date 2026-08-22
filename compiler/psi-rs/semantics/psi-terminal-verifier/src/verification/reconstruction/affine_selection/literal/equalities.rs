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

    pub(super) fn any(
        &self,
        mut candidate: impl FnMut(&'a Proposition, &'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        for equality in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (left, right) in [(left, right), (right, left)] {
                if candidate(equality, left, right) {
                    return true;
                }
            }
        }
        false
    }
}
