//! Exhaustive independent partition of operations eligible for dead-scalar removal.
use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleIdentity;
use semantic_vocabulary::{ScalarType, ValueId};

use super::rule_catalog::{DeadScalarFamily, dead_scalar_family};

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
        | O::IntegerBitwiseXor { .. } => Some(DeadScalarFamily::UnconditionallyTotal),
        O::IntegerExactCast { .. } => Some(DeadScalarFamily::ProofCertified),
        // An integer policy pair is total exactly when it carries no
        // obligation: wrapping and saturating add, subtract and multiply, and
        // wrapping shifts.
        O::IntegerShift { policy, .. } => Some(obligation_family(policy.obligation())),
        O::IntegerArithmetic { arithmetic, .. } => {
            Some(obligation_family(arithmetic.obligation()))
        }
        O::DynamicDescriptorParameter { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::AtomicEvent { .. }
        | O::EstablishScalarArray { .. } | O::EstablishStructuralArray { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishRecord { .. }
        | O::CallStructural { .. }
        | O::CallDynamic { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        // A re-view stays until its Terminal row joins a dead-scalar family:
        // only the producer's own partition may retire a verified operation.
        | O::ScalarRepresentationView { .. }
        | O::FloatResultPolicy { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequenceWrite { .. }
        | O::ByteSequenceStore { .. } | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceLength { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        // Restoration windows carry custody, not a scalar result a
        // dead-scalar rule may retire; the pair stays ineligible.
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        | O::StructuralLeafCopy { .. }
        // A Trapping primitive's crash is observable even when its result
        // is unused, so an unused instance is never dead.
        | O::TrappingInteger { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => None,
    }
}

fn obligation_family(obligation: Option<semantic_vocabulary::ObligationId>) -> DeadScalarFamily {
    match obligation {
        None => DeadScalarFamily::UnconditionallyTotal,
        Some(_) => DeadScalarFamily::ProofCertified,
    }
}

pub(crate) fn independently_validated_dead_scalar_shape(
    rule: OptimizationRuleIdentity,
    operation: &O,
) -> Option<(
    semantic_vocabulary::OperationId,
    ValueId,
    ScalarType,
    Option<semantic_vocabulary::ObligationId>,
)> {
    let rule_family = dead_scalar_family(rule)?;
    if independently_validated_dead_scalar_operation_family(operation) != Some(rule_family) {
        return None;
    }
    match (rule_family, operation) {
        (
            DeadScalarFamily::Literal,
            O::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                ..
            },
        ) => Some((*psi_operation, *result, *scalar_type, None)),
        (
            DeadScalarFamily::Literal,
            O::BooleanConstant {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            DeadScalarFamily::UnconditionallyTotal,
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
        ) => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            DeadScalarFamily::UnconditionallyTotal,
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
            },
        ) => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            None,
        )),
        (
            DeadScalarFamily::UnconditionallyTotal,
            O::IntegerWiden {
                psi_operation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            None,
        )),
        (
            DeadScalarFamily::ProofCertified,
            O::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                target_type,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*target_type),
            Some(*obligation),
        )),
        // The partition above already bound the family to the obligation's
        // presence, so the pair's own obligation is the shape's obligation.
        (
            DeadScalarFamily::UnconditionallyTotal | DeadScalarFamily::ProofCertified,
            O::IntegerShift {
                psi_operation,
                result,
                value_type,
                policy,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*value_type),
            policy.obligation(),
        )),
        (
            DeadScalarFamily::UnconditionallyTotal | DeadScalarFamily::ProofCertified,
            O::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                ..
            },
        ) => Some((
            *psi_operation,
            *result,
            ScalarType::Integer(*scalar_type),
            arithmetic.obligation(),
        )),
        _ => None,
    }
}
