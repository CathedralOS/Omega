//! Source-ordered direct retained-bound candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) struct DirectAffineCandidates<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> DirectAffineCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a Proposition) -> bool,
    ) -> bool {
        self.requirements
            .iter()
            .chain(self.semantic_axioms)
            .filter_map(|fact| match fact {
                Proposition::LessOrEqual(left, right) => Some((fact, left, right)),
                _ => None,
            })
            .any(|(root_bound, root_left, root_right)| {
                [root_left, root_right]
                    .into_iter()
                    .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                    .any(|root| complete(root, root_bound))
            })
    }
}
