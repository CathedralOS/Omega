//! Ordered affine-definition index recording for certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::candidates;

pub(super) fn by_input(semantic_axioms: &[Proposition]) -> BTreeMap<ScalarTerm, Vec<usize>> {
    let mut by_input = BTreeMap::<ScalarTerm, Vec<usize>>::new();
    candidates::visit(semantic_axioms, |index, input| {
        let candidates = by_input.entry(input.clone()).or_default();
        if candidates.last() != Some(&index) {
            candidates.push(index);
        }
    });
    by_input
}
