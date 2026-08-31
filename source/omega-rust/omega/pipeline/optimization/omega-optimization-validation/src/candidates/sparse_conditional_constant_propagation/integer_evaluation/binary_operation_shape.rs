//! Exhaustive recognition of binary integer operations eligible for SCCP replay.

use omega_abstract_operations::AbstractOperation as O;

use crate::OptimizationUnitValidationError;

use super::model::{BinaryIntegerOperation as K, BinaryOperationShape};

pub(super) fn recognize(
    operation: &O,
) -> Result<BinaryOperationShape, OptimizationUnitValidationError> {
    let shape = match operation {
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::ExactAdd,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::ExactSubtract,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::ExactMultiply,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::WrappingAdd,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::WrappingSubtract,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::WrappingMultiply,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::SaturatingAdd,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::SaturatingSubtract,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::SaturatingMultiply,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::ExactDivide,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::ExactRemainder,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::WrappingDivide,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::WrappingRemainder,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::SaturatingDivide,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => BinaryOperationShape {
            kind: K::SaturatingRemainder,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => BinaryOperationShape {
            kind: K::ExactShiftLeft(*count_type),
            source: *psi_operation,
            result: *result,
            scalar_type: *value_type,
            left: *value,
            right: *count,
        },
        O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => BinaryOperationShape {
            kind: K::ExactShiftRight(*count_type),
            source: *psi_operation,
            result: *result,
            scalar_type: *value_type,
            left: *value,
            right: *count,
        },
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => BinaryOperationShape {
            kind: K::WrappingShiftLeft(*count_type),
            source: *psi_operation,
            result: *result,
            scalar_type: *value_type,
            left: *value,
            right: *count,
        },
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => BinaryOperationShape {
            kind: K::WrappingShiftRight(*count_type),
            source: *psi_operation,
            result: *result,
            scalar_type: *value_type,
            left: *value,
            right: *count,
        },
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::BitwiseAnd,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::BitwiseOr,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::BitwiseXor,
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    Ok(shape)
}
