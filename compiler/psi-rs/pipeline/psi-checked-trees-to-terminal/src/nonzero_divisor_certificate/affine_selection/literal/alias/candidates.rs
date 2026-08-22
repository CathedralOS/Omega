//! Source-ordered one-alias literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod landing_index;

use super::super::{eligibility, equalities::OrientedEqualities};
use landing_index::LandingIndex;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(
        &'a ScalarTerm,
        &'a ScalarTerm,
        &'a ScalarTerm,
        ProofNode,
        ProofNode,
    ) -> Option<T>,
) -> Option<T> {
    let root_equalities = OrientedEqualities::new(assumptions, semantic_axioms);
    let landings = LandingIndex::new(assumptions, semantic_axioms);
    root_equalities
        .iter()
        .find_map(|(outer_citation, outer_equality, root, alias)| {
            if root == alias
                || !matches!(root, ScalarTerm::Value { .. })
                || !matches!(alias, ScalarTerm::Value { .. })
                || root.scalar_type() != alias.scalar_type()
            {
                return None;
            }
            for &(inner_citation, inner_equality, literal) in landings.candidates(alias) {
                if !eligibility::one_alias_join(outer_equality, root, inner_equality, literal) {
                    continue;
                }
                if let Some(result) = complete(
                    root,
                    alias,
                    literal,
                    outer_citation.proof(outer_equality),
                    inner_citation.proof(inner_equality),
                ) {
                    return Some(result);
                }
            }
            None
        })
}
