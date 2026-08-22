//! Source-ordered one-alias literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod join;
mod landing_index;
mod root_aliases;

use landing_index::LandingIndex;
use root_aliases::RootAliases;

pub(super) struct LiteralAliasCandidates<'a> {
    root_aliases: RootAliases<'a>,
    landings: LandingIndex<'a>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            root_aliases: RootAliases::new(requirements, semantic_axioms),
            landings: LandingIndex::new(requirements, semantic_axioms),
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
    ) -> bool {
        self.root_aliases.any(|outer_equality, root, alias| {
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
