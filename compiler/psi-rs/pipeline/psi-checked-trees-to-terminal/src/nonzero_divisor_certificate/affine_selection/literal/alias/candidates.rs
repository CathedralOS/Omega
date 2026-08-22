//! Source-ordered one-alias literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod landing_index;

use super::super::super::equalities;
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
    let landings = LandingIndex::new(assumptions, semantic_axioms);
    equalities::value_aliases(assumptions, semantic_axioms).find_map(
        |(outer_citation, outer_equality, root, alias)| {
            if root.scalar_type() != alias.scalar_type() {
                return None;
            }
            landings.find(alias, outer_equality, |literal, inner_proof| {
                complete(
                    root,
                    alias,
                    literal,
                    outer_citation.proof(outer_equality),
                    inner_proof,
                )
            })
        },
    )
}
