//! Source-ordered one-alias literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod landing_index;

use super::super::super::equalities;
use landing_index::LandingIndex;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    let landings = LandingIndex::new(requirements, semantic_axioms);
    equalities::value_aliases(requirements, semantic_axioms).any(|(outer_equality, root, alias)| {
        if root.scalar_type() != alias.scalar_type() {
            return false;
        }
        landings.any(alias, outer_equality, |literal| complete(root, literal))
    })
}
