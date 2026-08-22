//! Source-ordered oriented equalities for direct literal reconstruction.

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
        mut candidate: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        for equality in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (root, literal) in [(left, right), (right, left)] {
                if candidate(root, literal) {
                    return true;
                }
            }
        }
        false
    }
}
