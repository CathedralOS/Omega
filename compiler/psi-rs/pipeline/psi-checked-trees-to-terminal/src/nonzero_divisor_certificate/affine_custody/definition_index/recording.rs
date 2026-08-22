//! Ordered affine-definition index recording for certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::candidates;

/// Source-ordered semantic rows that may extend one exact affine value chain.
/// This selects candidates only; the kernel remains authoritative for every
/// prefix and completed proof.
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

    pub(in crate::nonzero_divisor_certificate::affine_custody) fn candidates_from(
        &self,
        input: &ScalarTerm,
        start: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let candidates = self.by_input.get(input).map(Vec::as_slice).unwrap_or(&[]);
        let first = candidates.partition_point(|&index| index < start);
        candidates[first..].iter().copied()
    }
}
