//! Exhaustive independent partition of operations eligible for dead-scalar removal.

use super::rule_catalog::{DeadScalarFamily, dead_scalar_family};
use super::*;

/// Independent exhaustive mirror of the producer's closed safety partition.
/// A new abstract operation cannot compile until this validator decides that
/// unused instances belong to one exact family or remain ineligible.
fn independently_validated_dead_scalar_operation_family(operation: &O) -> Option<DeadScalarFamily> {
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
        O::DynamicDescriptorParameter { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::CallUnit { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallStructural { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => None,
    }
}

pub(crate) fn independently_validated_dead_scalar_shape(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<(
    psi_core::OperationId,
    ValueId,
    ScalarType,
    Option<psi_core::ObligationId>,
)> {
    let rule_family = dead_scalar_family(rule)?;
    if independently_validated_dead_scalar_operation_family(operation) != Some(rule_family) {
        return None;
    }
    match (rule_family, operation) {
        (
            family,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) if family == DeadScalarFamily::Literal => {
            Some((*psi_operation, *result, *scalar_type, None))
        }
        (
            family,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) if family == DeadScalarFamily::Literal => {
            Some((*psi_operation, *result, ScalarType::Boolean, None))
        }
        (
            family,
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
            },
        ) if family == DeadScalarFamily::UnconditionallyTotal => {
            Some((*psi_operation, *result, ScalarType::Boolean, None))
        }
        (
            family,
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
            },
        ) if family == DeadScalarFamily::UnconditionallyTotal => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            None,
        )),
        (
            family,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) if family == DeadScalarFamily::UnconditionallyTotal => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            None,
        )),
        (
            family,
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
            },
        ) if family == DeadScalarFamily::UnconditionallyTotal => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            None,
        )),
        (
            family,
            O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                target_type,
                ..
            },
        ) if family == DeadScalarFamily::ProofCertified => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            Some(*obligation),
        )),
        (
            family,
            O::ExactIntegerShiftLeft {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            }
            | O::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                ..
            },
        ) if family == DeadScalarFamily::ProofCertified => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            Some(*obligation),
        )),
        (
            family,
            O::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerSubtract {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerDivide {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            }
            | O::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                ..
            },
        ) if family == DeadScalarFamily::ProofCertified => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            Some(*obligation),
        )),
        _ => None,
    }
}
