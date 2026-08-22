//! Invocation-local affine-definition candidate indexing.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

mod candidates;
mod recording;

/// Source-ordered semantic rows that can extend an affine definition chain
/// from one exact value.
///
/// The index is candidate pruning only. Every indexed word is still replayed
/// independently by the proof kernel before it advances the frontier.
pub(crate) struct DefinitionIndex {
    by_input: BTreeMap<ScalarTerm, Vec<usize>>,
}

impl DefinitionIndex {
    pub(crate) fn new(semantic_axioms: &[Proposition]) -> Self {
        Self {
            by_input: recording::by_input(semantic_axioms),
        }
    }

    pub(super) fn candidates(&self, input: &ScalarTerm) -> &[usize] {
        self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[])
    }
}
