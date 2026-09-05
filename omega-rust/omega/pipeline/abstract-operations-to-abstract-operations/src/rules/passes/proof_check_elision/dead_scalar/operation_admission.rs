//! Closed proof-bearing scalar operation admission for the exact dead-node rule.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::ScalarType;

use crate::rules::passes::support::DeadScalarShape;

pub(super) fn classify(operation: &O) -> Option<DeadScalarShape> {
    let (source_operation, result, scalar_type) = match operation {
        O::IntegerExactCast {
            psi_operation,
            result,
            target_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*target_type)),
        O::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            ..
        }
        | O::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*value_type)),
        O::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | O::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            ..
        } => (*psi_operation, *result, ScalarType::Integer(*scalar_type)),
        _ => return None,
    };
    Some(DeadScalarShape {
        source_operation,
        result,
        scalar_type,
    })
}
