//! Ordered prefix targets for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn values(definition: &Proposition) -> impl Iterator<Item = &ScalarTerm> {
    let endpoints = match definition {
        Proposition::Equal(left, right) => [Some(left), Some(right)],
        _ => unreachable!("definition index contains only equality rows"),
    };
    endpoints
        .into_iter()
        .flatten()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
}
