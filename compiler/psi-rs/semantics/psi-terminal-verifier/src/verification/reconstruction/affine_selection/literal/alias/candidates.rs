//! Source-ordered one-alias literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm, ScalarType};

mod landing_index;

use landing_index::LandingIndex;

pub(super) struct LiteralAliasCandidates<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    landings: LandingIndex<'a>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            requirements,
            semantic_axioms,
            landings: LandingIndex::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        for outer_equality in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::Equal(outer_left, outer_right) = outer_equality else {
                continue;
            };
            for (root, alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
                if root == alias
                    || !matches!(root, ScalarTerm::Value { .. })
                    || !matches!(alias, ScalarTerm::Value { .. })
                    || root.scalar_type() != alias.scalar_type()
                {
                    continue;
                }
                for &(inner_equality, literal) in self.landings.candidates(alias) {
                    if std::ptr::eq(outer_equality, inner_equality) {
                        continue;
                    }
                    let Some((integer_type, _)) = literal.integer_value() else {
                        unreachable!("literal index contains only integer landings")
                    };
                    if root.scalar_type() == ScalarType::Integer(integer_type)
                        && complete(root, literal)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}
