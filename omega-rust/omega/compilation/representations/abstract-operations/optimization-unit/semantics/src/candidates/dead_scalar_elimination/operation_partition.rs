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
        abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
        | O::IntegerWiden { .. }
        | O::IntegerBitwiseAnd { .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(..) => Some(DeadScalarFamily::UnconditionallyTotal),
        O::IntegerExactCast { .. } => Some(DeadScalarFamily::ProofCertified),
        // An integer policy pair is total exactly when it carries no
        // obligation: wrapping and saturating add, subtract and multiply, and
        // wrapping shifts.
        O::IntegerShift { policy, .. } => Some(obligation_family(policy.obligation())),
        O::IntegerArithmetic { arithmetic, .. } => {
            Some(obligation_family(arithmetic.obligation()))
        }
        O::SelectImplementation { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | abstract_operations::scalar_read_or_membership_pattern!(..)
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::AtomicEvent { .. }
        | O::EstablishScalarArray { .. } | abstract_operations::structural_array_or_case_pattern!(..)
        | abstract_operations::establish_byte_literal_triple_pattern!(..)
        | abstract_operations::structural_establish_triple_pattern!(..)
        | abstract_operations::boundary_or_structural_call_pair_pattern!(..)
        | O::PortWrite { .. }
        | O::Call { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        // A re-view stays until its Terminal row joins a dead-scalar family:
        // only the producer's own partition may retire a verified operation.
        | O::ScalarRepresentationView { .. }
        | O::FloatResultPolicy { .. }
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | O::ByteSequenceWrite { .. }
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | abstract_operations::byte_view_triple_pattern!(..)
        | abstract_operations::byte_view_length_triple_pattern!(..)
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | O::ElementViewSubslice { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | O::StructuralScalarFieldRead { .. }
        | O::EstablishReference { .. }
        | O::EstablishPlacedReference { .. }
        | O::ReleaseReference { .. }
        // Restoration windows carry custody, not a scalar result a
        // dead-scalar rule may retire; the pair stays ineligible.
        | abstract_operations::move_or_store_structural_field_pattern!()
        | O::StructuralLeafCopy { .. }
        // A Trapping primitive's crash is observable even when its result
        // is unused, so an unused instance is never dead.
        | O::TrappingInteger { .. }
        | abstract_operations::jump_or_conditional_pattern!()
        | O::StructuralCase { .. }
        | abstract_operations::return_or_return_unit_pattern!()
        | abstract_operations::return_structural_or_crash_pattern!() => None,
        O::ExternalCompletion { .. } => None,
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
            abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
            | abstract_operations::integer_equal_less_pair_pattern!(psi_operation, result, ..)
            | O::IntegerLessOrEqual {
                psi_operation,
                result,
                ..
            },
        ) => Some((*psi_operation, *result, ScalarType::Boolean, None)),
        (
            DeadScalarFamily::UnconditionallyTotal,
            abstract_operations::bitwise_not_and_pair_pattern!(
                psi_operation,
                result,
                scalar_type,
                ..
            )
            | abstract_operations::bitwise_or_xor_pair_pattern!(
                psi_operation,
                result,
                scalar_type,
                ..
            ),
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
