//! Source-ordered one-alias literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod join;
mod landing_index;

use super::super::equalities::OrientedEqualities;
use landing_index::LandingIndex;

pub(super) struct LiteralAliasCandidates<'a> {
    root_equalities: OrientedEqualities<'a>,
    landings: LandingIndex<'a>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            root_equalities: OrientedEqualities::new(requirements, semantic_axioms),
            landings: LandingIndex::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.root_equalities
            .iter()
            .any(|(outer_equality, root, alias)| {
                if root == alias
                    || !matches!(root, ScalarTerm::Value { .. })
                    || !matches!(alias, ScalarTerm::Value { .. })
                    || root.scalar_type() != alias.scalar_type()
                {
                    return false;
                }
                for &(inner_equality, literal) in self.landings.candidates(alias) {
                    if join::eligible(outer_equality, root, inner_equality, literal)
                        && complete(root, literal)
                    {
                        return true;
                    }
                }
                false
            })
    }
}
