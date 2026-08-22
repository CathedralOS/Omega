//! Oriented affine-definition expressions for certificate production.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn value_target_expressions(
    proposition: &Proposition,
) -> impl Iterator<Item = &ScalarTerm> {
    let orientations = match proposition {
        Proposition::Equal(left, right) => [Some((left, right)), Some((right, left))],
        _ => [None, None],
    };
    orientations
        .into_iter()
        .flatten()
        .filter_map(|(target, expression)| {
            matches!(target, ScalarTerm::Value { .. }).then_some(expression)
        })
}
