use omega_abstract_operations::AbstractOperation as O;
use psi_core::{OperationId, ScalarType, ValueId};

use super::family::DeadScalarFamily;

/// Exhaustively partitions the complete abstract-operation vocabulary. Adding
/// an operation is therefore a compile-time request to decide whether unused
/// instances are literals, total scalar work, proof-certified work, or not
/// removable by these suites.
fn dead_scalar_family(operation: &O) -> Option<DeadScalarFamily> {
    match operation {
        O::IntegerConstant { .. } | O::BooleanConstant { .. } => Some(DeadScalarFamily::Literal),
        O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::SaturatingIntegerMultiply { .. } => Some(DeadScalarFamily::UnconditionallyTotal),
        O::IntegerExactCast { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::ExactIntegerAdd { .. }
        | O::ExactIntegerSubtract { .. }
        | O::ExactIntegerMultiply { .. }
        | O::ExactIntegerDivide { .. }
        | O::ExactIntegerRemainder { .. }
        | O::WrappingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::SaturatingIntegerRemainder { .. } => Some(DeadScalarFamily::ProofCertified),
        O::WriteOnlyPrimitiveStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. }
        | O::BooleanStructuralField { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => None,
    }
}

pub(super) fn dead_scalar_shape(
    operation: &O,
    family: DeadScalarFamily,
) -> Option<(OperationId, ValueId, ScalarType)> {
    if dead_scalar_family(operation) != Some(family) {
        return None;
    }
    match operation {
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            ..
        } => Some((*psi_operation, *result, *scalar_type)),
        O::BooleanConstant {
            psi_operation,
            result,
            ..
        }
        | O::BooleanNot {
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
        } => Some((*psi_operation, *result, ScalarType::Boolean)),
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
        } => Some((*psi_operation, *result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            psi_operation,
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            psi_operation,
            result,
            target_type,
            ..
        } => Some((*psi_operation, *result, ScalarType::Integer(*target_type))),
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
        }
        | O::ExactIntegerShiftLeft {
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
        } => Some((*psi_operation, *result, ScalarType::Integer(*value_type))),
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
        } => Some((*psi_operation, *result, ScalarType::Integer(*scalar_type))),
        O::WriteOnlyPrimitiveStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructural { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. }
        | O::BooleanStructuralField { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => None,
    }
}
