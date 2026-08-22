//! Invocation-local affine-definition candidate indexing.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

mod candidates;

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
        let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        candidates::visit(semantic_axioms, |index, input| {
            let candidates = by_input.entry(input.clone()).or_default();
            if candidates.last() != Some(&index) {
                candidates.push(index);
            }
        });
        Self { by_input }
    }

    pub(super) fn candidates(&self, input: &ScalarTerm) -> &[usize] {
        self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[])
    }
}
