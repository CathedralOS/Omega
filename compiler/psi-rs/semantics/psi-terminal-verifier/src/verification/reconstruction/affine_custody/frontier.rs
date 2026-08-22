//! Fixed affine-witness candidate frontier for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::DefinitionIndex;

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
    let mut frontier = vec![(Vec::new(), 0, root.clone())];
    for _ in 0..MAX_DEFINITIONS {
        let mut next = Vec::new();
        for (prefix, start, current) in frontier {
            for &index in definitions
                .candidates(&current)
                .iter()
                .skip_while(|&&index| index < start)
            {
                let mut word = prefix.clone();
                word.push(index);
                if let Some(next_target) = prefix::checked_target(
                    context,
                    semantic_axioms,
                    root,
                    &word,
                    &semantic_axioms[index],
                ) {
                    words.push(word.clone());
                    next.push((word, index + 1, next_target.clone()));
                }
            }
        }
        frontier = next;
    }
    words
}
