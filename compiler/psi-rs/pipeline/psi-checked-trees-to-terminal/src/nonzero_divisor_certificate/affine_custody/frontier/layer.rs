//! One source-ordered affine frontier layer for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::DefinitionIndex;
use super::prefix;

pub(super) struct Entry {
    word: Vec<usize>,
    start: usize,
    current: ScalarTerm,
}

impl Entry {
    pub(super) fn root(root: &ScalarTerm) -> Self {
        Self {
            word: Vec::new(),
            start: 0,
            current: root.clone(),
        }
    }
}

pub(super) fn expand(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    frontier: Vec<Entry>,
    words: &mut Vec<Vec<usize>>,
) -> Vec<Entry> {
    let mut next = Vec::new();
    for entry in frontier {
        for &index in definitions
            .candidates(&entry.current)
            .iter()
            .skip_while(|&&index| index < entry.start)
        {
            let mut word = entry.word.clone();
            word.push(index);
            if let Some(next_target) = prefix::checked_target(
                context,
                semantic_axioms,
                root,
                &word,
                &semantic_axioms[index],
            ) {
                words.push(word.clone());
                next.push(Entry {
                    word,
                    start: index + 1,
                    current: next_target.clone(),
                });
            }
        }
    }
    next
}
