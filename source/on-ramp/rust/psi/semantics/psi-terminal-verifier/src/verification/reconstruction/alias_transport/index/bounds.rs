//! Retained endpoint index for independent alias-bound reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

pub(in super::super) fn indexed_bounds<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(&'a Proposition, usize)>> {
    let mut bounds_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for fact in requirements.iter().chain(semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((fact, 0));
        }
        if right != left && matches!(right, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((fact, 1));
        }
    }
    bounds_by_endpoint
}
