//! Fixed affine-witness candidate frontier for certificate production.

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

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves affine custody.
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
