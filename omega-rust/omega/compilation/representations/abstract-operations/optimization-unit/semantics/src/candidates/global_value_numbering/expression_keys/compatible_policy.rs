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
    let (key, psi_operation, result, scalar_type, obligation) =
        compatible_policy_scalar_row(operation)?;
    obligation.is_none().then_some((
        IndependentScalarExpressionKey::CompatiblePolicy(key),
        psi_operation,
        result,
        scalar_type,
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
    let (key, psi_operation, result, scalar_type, obligation) =
        compatible_policy_scalar_row(operation)?;
    obligation.map(|obligation| {
        (
            IndependentScalarExpressionKey::CompatiblePolicy(key),
            psi_operation,
            result,
            scalar_type,
            Some(obligation),
        )
    })
}

/// The compatible-policy row one operation offers: wrapping and saturating
/// policies carry no obligation (leader shape), exact policies carry theirs
/// (redundant shape); any other policy has no compatible-policy row.
fn compatible_policy_scalar_row(
    operation: &O,
) -> Option<(
    IndependentCompatiblePolicyScalarExpressionKey,
    OperationId,
    ValueId,
    ScalarType,
    Option<semantic_vocabulary::ObligationId>,
)> {
    match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } => {
            let obligation = match policy {
                IntegerShiftPolicy::Wrapping => None,
                IntegerShiftPolicy::Exact { obligation } => Some(*obligation),
            };
            Some((
                shift_key(*direction, *value_type, *count_type, *value, *count),
                *psi_operation,
                *result,
                ScalarType::Integer(*value_type),
                obligation,
            ))
        }
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => {
            let obligation = match arithmetic.policy() {
                IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Saturating => None,
                IntegerOverflowPolicy::Exact => Some(arithmetic.obligation()?),
            };
            Some((
                arithmetic_key(arithmetic.operator(), *scalar_type, *left, *right)?,
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                obligation,
            ))
        }
        _ => None,
    }
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
