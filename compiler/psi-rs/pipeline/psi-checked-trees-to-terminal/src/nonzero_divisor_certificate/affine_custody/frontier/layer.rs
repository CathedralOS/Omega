//! One source-ordered affine frontier layer for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::DefinitionIndex;
use super::prefix;

mod entry;

pub(super) use entry::Entry;

pub(super) fn expand(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    frontier: Vec<Entry>,
    words: &mut Vec<Vec<usize>>,
    retain_successors: bool,
) -> Vec<Entry> {
    let mut next = Vec::new();
    for entry in frontier {
        for (index, word) in entry.extensions(definitions) {
            if let Some(next_target) = prefix::checked_target(
                context,
                semantic_axioms,
                root,
                &word,
                &semantic_axioms[index],
            ) {
                if retain_successors {
                    words.push(word.clone());
                    next.push(Entry::advance(word, index, next_target));
                } else {
                    words.push(word);
                }
            }
        }
    }
    next
}
