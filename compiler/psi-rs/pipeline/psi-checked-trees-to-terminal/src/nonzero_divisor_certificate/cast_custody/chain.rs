//! Exact-cast definition-spine selection for certificate production.

use psi_core::{Proposition, ScalarTerm};

mod definitions;
mod source;

/// Follow the exact SSA definition spine backward from `target` to `root`.
///
/// This is intentionally not graph search: every reached target must have one
/// exact-cast definition, and the resulting source-ordered word must already be
/// canonical in the semantic ledger.
pub(super) fn definition_axioms(
    root: &ScalarTerm,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<Vec<usize>> {
    definitions::axioms(root, target, semantic_axioms)
}

/// Recover the unique non-cast source and the first cast's ledger position.
///
/// The walk is a single exact SSA spine, not a graph search. It stops at the
/// first value without an exact-cast definition and rejects ambiguous,
/// cyclic, or non-source-ordered cast words.
pub(super) fn source_root(
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<(ScalarTerm, usize)> {
    source::root(target, semantic_axioms)
}
