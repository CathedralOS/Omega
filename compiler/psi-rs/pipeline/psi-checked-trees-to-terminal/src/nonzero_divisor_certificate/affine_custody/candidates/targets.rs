//! Ordered affine goal targets for certificate production.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn values(goal: &Proposition) -> impl Iterator<Item = &ScalarTerm> {
    let endpoints = match goal {
        Proposition::LessOrEqual(left, right) => [Some(left), Some(right)],
        _ => [None, None],
    };
    endpoints
        .into_iter()
        .flatten()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
}
