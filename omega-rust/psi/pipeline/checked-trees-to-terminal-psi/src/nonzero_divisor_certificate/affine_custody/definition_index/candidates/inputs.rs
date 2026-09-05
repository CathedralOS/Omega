//! Affine-definition input projection for certificate production.

use semantic_vocabulary::ScalarTerm;

pub(super) fn affine_values(expression: &ScalarTerm) -> impl Iterator<Item = &ScalarTerm> {
    let inputs = match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    };
    inputs
        .into_iter()
        .flatten()
        .filter(|input| matches!(input, ScalarTerm::Value { .. }))
}
