//! Affine-definition input projection for certificate production.

use semantic_vocabulary::ScalarTerm;

pub(super) fn affine_values(expression: &ScalarTerm) -> impl Iterator<Item = &ScalarTerm> {
    let inputs = match expression {
        // A bitwise and resumes a chain at either operand; the kernel admits
        // the reading only when the sibling operand lands as a non-negative
        // mask literal.
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        // Only the dividend of a wrapping division resumes a chain; the
        // divisor must resolve as a literal and never carries a bound.
        ScalarTerm::ExactIntegerSubtract { left, .. }
        | ScalarTerm::WrappingIntegerDivide { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    };
    inputs
        .into_iter()
        .flatten()
        .filter(|input| matches!(input, ScalarTerm::Value { .. }))
}
