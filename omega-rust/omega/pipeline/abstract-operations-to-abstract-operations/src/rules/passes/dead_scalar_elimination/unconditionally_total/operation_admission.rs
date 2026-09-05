//! Closed unconditionally-total scalar operation admission for the exact rule.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::ScalarType;

use crate::rules::passes::support::DeadScalarShape;

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        O::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | O::BooleanEqual {
            psi_operation,
            result,
            ..
        }
        | O::IntegerEqual {
            psi_operation,
            result,
            ..
        }
        | O::IntegerLessThan {
            psi_operation,
            result,
            ..
        }
        | O::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result, ScalarType::Boolean),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*scalar_type)),
        O::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
        _ => return None,
    };
    Some(DeadScalarShape {
        source_operation,
        result,
        scalar_type,
    })
}
