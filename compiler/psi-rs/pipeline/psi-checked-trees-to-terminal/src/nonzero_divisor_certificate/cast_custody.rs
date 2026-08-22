//! Side-local custody facade for an exact integer-cast chain.

mod chain;
mod completion;
mod literal;

pub(super) use completion::prove_from_root;
pub(super) use literal::remap_integer_literal;

pub(super) fn source_root(
    target: &psi_core::ScalarTerm,
    semantic_axioms: &[psi_core::Proposition],
) -> Option<(psi_core::ScalarTerm, usize)> {
    chain::source_root(target, semantic_axioms)
}

pub(super) fn definition_axioms(
    root: &psi_core::ScalarTerm,
    target: &psi_core::ScalarTerm,
    semantic_axioms: &[psi_core::Proposition],
) -> Option<Vec<usize>> {
    chain::definition_axioms(root, target, semantic_axioms)
}
