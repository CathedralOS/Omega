//! Independent exact-cast definition-spine reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod definitions;
mod source;

/// Reconstruct the unique exact-cast SSA definition spine.
///
/// This follows one definition per reached target and never explores alternate
/// paths or permutations. The proof-kernel witness checker still owns all cast
/// legality, continuity, and carrier validation.
pub(super) fn definition_axioms(
    root: &ScalarTerm,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<Vec<usize>> {
    definitions::axioms(root, target, semantic_axioms)
}

/// Independently recover the unique non-cast source and first cast index.
pub(super) fn source_root(
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<(ScalarTerm, usize)> {
    source::root(target, semantic_axioms)
}
