//! Compatible-policy leader and redundant-operation classification.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{IntegerType, ScalarType, ValueId};

use super::{CompatiblePolicyScalarExpressionKey, ScalarExpressionRow, canonical_pair};

pub(in crate::rules) fn compatible_policy_scalar_leader(
    operation: &O,
) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
    compatible_policy_row(
        operation,
        |policy| policy == IntegerShiftPolicy::Wrapping,
        |policy| {
            matches!(
                policy,
                IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating
            )
        },
    )
}

pub(in crate::rules) fn compatible_policy_scalar_redundant(
    operation: &O,
) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
    compatible_policy_row(
        operation,
        |policy| matches!(policy, IntegerShiftPolicy::Exact { .. }),
        |policy| policy == IntegerOverflowPolicy::Exact,
    )
}

/// The policy-independent row of the operation, or `None` when its shift and
/// arithmetic policies are outside the calling family's admitted sets.
fn compatible_policy_row(
    operation: &O,
    admits_shift: impl Fn(IntegerShiftPolicy) -> bool,
    admits_arithmetic: impl Fn(IntegerOverflowPolicy) -> bool,
) -> Option<ScalarExpressionRow<CompatiblePolicyScalarExpressionKey>> {
    let row = match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } if admits_shift(*policy) => (
            shift_key(*direction, *value_type, *count_type, *value, *count),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if admits_arithmetic(arithmetic.policy()) => (
            arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        _ => return None,
    };
    Some(row)
}

fn shift_key(
    direction: IntegerShiftDirection,
    value_type: IntegerType,
    count_type: IntegerType,
    value: ValueId,
    count: ValueId,
) -> CompatiblePolicyScalarExpressionKey {
    match direction {
        IntegerShiftDirection::Left => {
            CompatiblePolicyScalarExpressionKey::ShiftLeft(value_type, count_type, value, count)
        }
        IntegerShiftDirection::Right => {
            CompatiblePolicyScalarExpressionKey::ShiftRight(value_type, count_type, value, count)
        }
    }
}

/// The policy-independent key of add, subtract and multiply; divide and
/// remainder have no compatible-policy leader.
fn arithmetic_key(
    operator: IntegerArithmeticOperator,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
) -> Option<CompatiblePolicyScalarExpressionKey> {
    let (low, high) = canonical_pair(left, right);
    match operator {
        IntegerArithmeticOperator::Add => Some(CompatiblePolicyScalarExpressionKey::Add(
            scalar_type,
            low,
            high,
        )),
        IntegerArithmeticOperator::Subtract => Some(CompatiblePolicyScalarExpressionKey::Subtract(
            scalar_type,
            left,
            right,
        )),
        IntegerArithmeticOperator::Multiply => Some(CompatiblePolicyScalarExpressionKey::Multiply(
            scalar_type,
            low,
            high,
        )),
        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => None,
    }
}
