//! Fixed affine-witness candidate frontier for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::DefinitionIndex;

mod layer;
mod prefix;

pub(super) fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This only prunes candidate words. Every retained prefix and final bound
    // is independently replayed by the reconstruction checkers.
    let mut words = Vec::new();
    let mut frontier = vec![layer::Entry::root(root)];
    for depth in 0..MAX_DEFINITIONS {
        frontier = layer::expand(
            context,
            semantic_axioms,
            definitions,
            root,
            frontier,
            &mut words,
            depth + 1 < MAX_DEFINITIONS,
        );
        if frontier.is_empty() {
            break;
        }
    }
    words
}

pub(super) fn literal_axioms(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    prefix::literal_axioms(context, semantic_axioms, root, definition_axioms, target)
}
