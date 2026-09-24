//! Ordered prefix targets for affine certificate production.

use semantic_vocabulary::{Proposition, ScalarTerm};

/// The values a definition can resume a chain at: its `Value` endpoints cover
/// the forward direction, and each `Value` addend of an add side covers the
/// checked backward direction toward that operand.
pub(super) fn values(definition: &Proposition) -> impl Iterator<Item = &ScalarTerm> {
    let endpoints = match definition {
        Proposition::Equal(left, right) => [left, right],
        _ => unreachable!("definition index contains only equality rows"),
    };
    endpoints.into_iter().flat_map(|side| {
        let mut targets: Vec<&ScalarTerm> = Vec::with_capacity(3);
        if matches!(side, ScalarTerm::Value { .. }) {
            targets.push(side);
        }
        if let ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. } = side
        {
            for operand in [left.as_ref(), right.as_ref()] {
                if matches!(operand, ScalarTerm::Value { .. }) {
                    targets.push(operand);
                }
            }
        }
        targets
    })
}
