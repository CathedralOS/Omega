//! Directional compatible-policy leader and redundant reconstruction.

use super::*;

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
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
            )
        }
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
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
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftLeft(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::ShiftRight(
                *value_type,
                *count_type,
                *value,
                *count,
            ),
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            *obligation,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Add(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => (
            IndependentCompatiblePolicyScalarExpressionKey::Subtract(*scalar_type, *left, *right),
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            *obligation,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let (left, right) = independent_pair(*left, *right);
            (
                IndependentCompatiblePolicyScalarExpressionKey::Multiply(*scalar_type, left, right),
                *psi_operation,
                *result,
                ScalarType::Integer(*scalar_type),
                *obligation,
            )
        }
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
