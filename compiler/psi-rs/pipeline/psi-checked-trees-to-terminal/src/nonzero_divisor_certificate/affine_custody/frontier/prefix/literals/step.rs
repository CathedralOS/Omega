//! Producer-local landed affine-sibling step selection.

use psi_core::{ScalarTerm, ScalarType};

pub(super) fn select<'a>(
    target: &'a ScalarTerm,
    expression: &'a ScalarTerm,
    current: &ScalarTerm,
    expected: ScalarType,
) -> Option<(&'a ScalarTerm, &'a ScalarTerm)> {
    if !matches!(target, ScalarTerm::Value { .. }) || target.scalar_type() != expected {
        return None;
    }
    let (left, right, subtraction) = match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            (left.as_ref(), right.as_ref(), false)
        }
        ScalarTerm::ExactIntegerSubtract { left, right, .. } => {
            (left.as_ref(), right.as_ref(), true)
        }
        _ => return None,
    };
    if expression.scalar_type() != expected {
        return None;
    }
    if left == current {
        Some((target, right))
    } else if !subtraction && right == current {
        Some((target, left))
    } else {
        None
    }
}
