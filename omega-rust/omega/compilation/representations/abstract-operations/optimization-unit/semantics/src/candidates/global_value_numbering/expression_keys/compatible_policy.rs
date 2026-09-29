//! Directional compatible-policy leader and redundant reconstruction.
use crate::candidates::global_value_numbering::expression_keys::{
    IndependentCompatiblePolicyScalarExpressionKey, IndependentScalarExpressionKey,
    independent_pair,
};
use abstract_operations::{
    AbstractOperation as O, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use semantic_vocabulary::{IntegerType, OperationId, ScalarType, ValueId};

pub(crate) fn independent_compatible_policy_scalar_leader(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<semantic_vocabulary::ObligationId>,
)> {
    let row = match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Wrapping,
            direction,
            value,
            count,
        } => (
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
        } if matches!(
            arithmetic.policy(),
            IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating
        ) =>
        {
            (
                arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        None,
    ))
}

pub(crate) fn independent_compatible_policy_scalar_redundant(
    operation: &O,
) -> Option<(
    IndependentScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<semantic_vocabulary::ObligationId>,
)> {
    let row = match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy: IntegerShiftPolicy::Exact { obligation },
            direction,
            value,
            count,
        } => (
            shift_key(*direction, *value_type, *count_type, *value, *count),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } if arithmetic.policy() == IntegerOverflowPolicy::Exact => (
            arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            arithmetic.obligation()?,
        ),
        _ => return None,
    };
    Some((
        IndependentScalarExpressionKey::CompatiblePolicy(row.0),
        row.1,
        row.2,
        row.3,
        Some(row.4),
    ))
}

fn shift_key(
    direction: IntegerShiftDirection,
    value_type: IntegerType,
    count_type: IntegerType,
    value: ValueId,
    count: ValueId,
) -> IndependentCompatiblePolicyScalarExpressionKey {
    match direction {
        IntegerShiftDirection::Left => IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
            value_type, count_type, value, count,
        ),
        IntegerShiftDirection::Right => IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
            value_type, count_type, value, count,
        ),
    }
}

/// The policy-independent key of add, subtract and multiply; divide and
/// remainder have no compatible-policy leader.
fn arithmetic_key(
    operator: IntegerArithmeticOperator,
    scalar_type: IntegerType,
    left: ValueId,
    right: ValueId,
) -> Option<IndependentCompatiblePolicyScalarExpressionKey> {
    let (low, high) = independent_pair(left, right);
    match operator {
        IntegerArithmeticOperator::Add => Some(
            IndependentCompatiblePolicyScalarExpressionKey::Add(scalar_type, low, high),
        ),
        IntegerArithmeticOperator::Subtract => Some(
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(scalar_type, left, right),
        ),
        IntegerArithmeticOperator::Multiply => Some(
            IndependentCompatiblePolicyScalarExpressionKey::Multiply(scalar_type, low, high),
        ),
        IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder => None,
    }
}
