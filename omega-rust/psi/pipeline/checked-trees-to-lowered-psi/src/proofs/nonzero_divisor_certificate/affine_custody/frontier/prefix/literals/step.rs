//! Producer-local landed affine-sibling step selection.

use semantic_vocabulary::{IntegerSign, ScalarTerm, ScalarType};

/// Every `(next, sibling)` reading one definition side admits for `current`.
/// A forward reading resumes at `target` and cites the other operand as the
/// landed sibling. An add reading traversed toward its operand resumes at
/// that operand instead; two `Value` addends yield both readings and the
/// caller keeps whichever sibling lands uniquely.
pub(super) fn select<'a>(
    target: &'a ScalarTerm,
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Vec<(&'a ScalarTerm, &'a ScalarTerm)> {
    let mut readings = Vec::with_capacity(2);
    if !matches!(target, ScalarTerm::Value { .. }) || target.scalar_type() != expected {
        return readings;
    }
    if let Some(sibling) = forward_sibling(expression, current, expected) {
        readings.push((target, sibling));
    }
    if target == current {
        readings.extend(add_operands(expression, expected));
    }
    readings
}

fn forward_sibling<'a>(
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Option<&'a ScalarTerm> {
    let unsigned = matches!(
        expected,
        ScalarType::Integer(integer_type) if integer_type.sign() == IntegerSign::Unsigned
    );
    let (left, right, subtraction) = match expression {
        // A bitwise and reads like a commutative operand pair: the current
        // value resumes at either side and the other side is the sibling
        // mask the literal search must land.
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. } => {
            (left.as_ref(), right.as_ref(), false)
        }
        ScalarTerm::WrappingIntegerAdd { left, right, .. } if unsigned => {
            (left.as_ref(), right.as_ref(), false)
        }
        ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
            (left.as_ref(), right.as_ref(), true)
        }
        ScalarTerm::WrappingIntegerDivide { left, right, .. } if unsigned => {
            (left.as_ref(), right.as_ref(), true)
        }
        _ => return None,
    };
    if expression.scalar_type() != expected {
        return None;
    }
    if left == current {
        Some(right)
    } else if !subtraction && right == current {
        Some(left)
    } else {
        None
    }
}

fn add_operands(expression: &ScalarTerm, expected: ScalarType) -> Vec<(&ScalarTerm, &ScalarTerm)> {
    let mut readings = Vec::new();
    let unsigned = matches!(
        expected,
        ScalarType::Integer(integer_type) if integer_type.sign() == IntegerSign::Unsigned
    );
    let (left, right) = match expression {
        ScalarTerm::WrappingIntegerAdd { left, right, .. } if unsigned => {
            (left.as_ref(), right.as_ref())
        }
        ScalarTerm::ExactIntegerAdd { left, right, .. } => (left.as_ref(), right.as_ref()),
        _ => return readings,
    };
    if expression.scalar_type() != expected {
        return readings;
    }
    for (operand, sibling) in [(left, right), (right, left)] {
        if matches!(operand, ScalarTerm::Value { .. }) {
            readings.push((operand, sibling));
        }
    }
    readings
}
