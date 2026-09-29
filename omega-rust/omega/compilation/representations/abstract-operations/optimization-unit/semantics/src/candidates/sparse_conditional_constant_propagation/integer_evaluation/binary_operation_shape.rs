//! Exhaustive recognition of binary integer operations eligible for SCCP replay.

use abstract_operations::AbstractOperation as O;

use crate::OptimizationUnitValidationError;

use super::{BinaryIntegerOperation as K, BinaryOperationShape};

pub(super) fn recognize(
    operation: &O,
) -> Result<BinaryOperationShape, OptimizationUnitValidationError> {
    let shape = match operation {
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => BinaryOperationShape {
            kind: K::Arithmetic(arithmetic.policy(), arithmetic.operator()),
            source: *psi_operation,
            result: *result,
            scalar_type: *scalar_type,
            left: *left,
            right: *right,
        },
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } => BinaryOperationShape {
            kind: K::Shift(*policy, *direction, *count_type),
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
