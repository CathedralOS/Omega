//! Source-ordered direct landed-literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm, ScalarType};

pub(super) struct DirectLiteralCandidates<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> DirectLiteralCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.requirements
            .iter()
            .chain(self.semantic_axioms)
            .filter_map(|equality| match equality {
                Proposition::Equal(left, right) => Some((left, right)),
                _ => None,
            })
            .any(|(left, right)| {
                [(left, right), (right, left)]
                    .into_iter()
                    .filter(|(root, literal)| {
                        matches!(root, ScalarTerm::Value { .. })
                            && literal.integer_value().is_some_and(|(integer_type, _)| {
                                root.scalar_type() == ScalarType::Integer(integer_type)
                            })
                    })
                    .any(|(root, literal)| complete(root, literal))
            })
    }
}
