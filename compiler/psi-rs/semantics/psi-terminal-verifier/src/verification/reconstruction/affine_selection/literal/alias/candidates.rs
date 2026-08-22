//! Source-ordered one-alias literal candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod landing_index;

use super::super::super::{eligibility, equalities};
use landing_index::LandingIndex;

pub(super) fn any<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm) -> bool,
) -> bool {
    let landings = LandingIndex::new(requirements, semantic_axioms);
    equalities::ordered(requirements, semantic_axioms).any(|(outer_equality, root, alias)| {
        if !eligibility::distinct_value_alias(root, alias)
            || root.scalar_type() != alias.scalar_type()
        {
            return false;
        }
        for &(inner_equality, literal) in landings.candidates(alias) {
            if eligibility::one_alias_join(outer_equality, root, inner_equality, literal)
                && complete(root, literal)
            {
                return true;
            }
        }
        false
    })
}
