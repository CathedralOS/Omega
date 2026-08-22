//! Producer-local affine-definition candidate indexing.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

/// Source-ordered semantic rows that may extend one exact affine value chain.
/// This selects candidates only; the kernel remains authoritative for every
/// prefix and completed proof.
pub(crate) struct DefinitionIndex {
    by_input: BTreeMap<ScalarTerm, Vec<usize>>,
}

impl DefinitionIndex {
    pub(crate) fn new(semantic_axioms: &[Proposition]) -> Self {
        let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
        for (index, proposition) in semantic_axioms.iter().enumerate() {
            let Proposition::Equal(left, right) = proposition else {
                continue;
            };
            for (target, expression) in [(left, right), (right, left)] {
                if !matches!(target, ScalarTerm::Value { .. }) {
                    continue;
                }
                for input in affine_inputs(expression).into_iter().flatten() {
                    if !matches!(input, ScalarTerm::Value { .. }) {
                        continue;
                    }
                    let candidates = by_input.entry(input.clone()).or_default();
                    if candidates.last() != Some(&index) {
                        candidates.push(index);
                    }
                }
            }
        }
        Self { by_input }
    }

    pub(super) fn candidates(&self, input: &ScalarTerm) -> &[usize] {
        self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn affine_inputs(expression: &ScalarTerm) -> [Option<&ScalarTerm>; 2] {
    match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    }
}
