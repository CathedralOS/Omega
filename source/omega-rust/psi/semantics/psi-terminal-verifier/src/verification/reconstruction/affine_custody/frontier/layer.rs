//! One source-ordered affine frontier layer for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::DefinitionIndex;
use super::prefix;

mod entry;

pub(super) use entry::Entry;

pub(super) fn expand(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    frontier: Vec<Entry>,
    words: &mut Vec<Vec<usize>>,
    retain_successors: bool,
) -> Vec<Entry> {
    let mut next = Vec::new();
    for entry in frontier {
        let extensions = entry.extensions(definitions).collect::<Vec<_>>();
        for (index, word) in extensions {
            if let Some(next_target) = prefix::checked_target(
                context,
                semantic_axioms,
                definitions,
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
