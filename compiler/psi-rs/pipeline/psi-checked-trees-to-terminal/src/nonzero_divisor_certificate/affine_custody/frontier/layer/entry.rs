//! Affine frontier cursor custody for certificate production.

use psi_core::ScalarTerm;

use super::super::super::DefinitionIndex;

pub(in crate::nonzero_divisor_certificate::affine_custody::frontier) struct Entry {
    word: Vec<usize>,
    start: usize,
    current: ScalarTerm,
}

impl Entry {
    pub(in crate::nonzero_divisor_certificate::affine_custody::frontier) fn root(
        root: &ScalarTerm,
    ) -> Self {
        Self {
            word: Vec::new(),
            start: 0,
            current: root.clone(),
        }
    }

    pub(super) fn extensions<'a>(
        &'a self,
        definitions: &'a DefinitionIndex,
    ) -> impl Iterator<Item = (usize, Vec<usize>)> + 'a {
        definitions
            .candidates_from(&self.current, self.start)
            .map(|index| {
                let mut word = self.word.clone();
                word.push(index);
                (index, word)
            })
    }

    pub(super) fn advance(word: Vec<usize>, index: usize, current: &ScalarTerm) -> Self {
        Self {
            word,
            start: index + 1,
            current: current.clone(),
        }
    }
}
