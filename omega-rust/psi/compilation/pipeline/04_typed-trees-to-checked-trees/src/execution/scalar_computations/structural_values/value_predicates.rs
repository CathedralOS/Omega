//! Structural carrier compatibility and fresh operand establishment.
//!
//! Receiving qualifications do not change a producer's storage carrier.
//! Zeroed scalar-array parts and borrowed argument operands retain their own
//! establishment rules; assignment planning uses the common value graph.

use super::super::{PrimitiveType, TypeReferenceNode};
use checked_trees::CheckedStructuralValueKind;
use typed_trees::types::TypeReferenceHandle;

/// A producer supplies its declared storage carrier; the receiving qualification
/// is a separate occurrence-bound checking judgment. Keeping the full receiving
/// type on the value node preserves that judgment without requiring constructors
/// or callees to repeat an already checked receiving membership.
pub(super) fn structural_carriers_match(
    program: &dyn ::validation::CheckingInput,
    returned: TypeReferenceHandle,
    expected: TypeReferenceHandle,
) -> bool {
    let (Some(returned), Some(expected)) = (
        validation::unwrapped_type_reference(program, returned),
        validation::unwrapped_type_reference(program, expected),
    ) else {
        return false;
    };
    program.normalized_type_identity(returned) == program.normalized_type_identity(expected)
}

/// An omitted runtime field's zero value is admitted only for a
/// `[scalar; N]` leaf: a literal-length fixed array whose element resolves
/// to one plain integer primitive. Anything else — borrowed carriers,
/// bounded integers, nested composites — has no authored zero spelling to
/// synthesize, so the field must stay authored.
pub(super) fn zeroed_scalar_array_parts(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<(PrimitiveType, u64)> {
    let reference = validation::unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let primitive_type = program.primitive_type_reference(*element_type)?;
    if !primitive_type.is_fixed_integer() {
        return None;
    }
    Some((primitive_type, u64::try_from(*length).ok()?))
}

/// The checked literal-zero computation one synthesized zeroed field
/// element contributes: a landed `0` in the declared element carrier.
pub(super) fn zeroed_integer_literal(
    primitive_type: PrimitiveType,
) -> Option<checked_trees::CheckedScalarExpression> {
    let landed_type = match primitive_type {
        PrimitiveType::I8 => numerics::literals::LandedIntegerType::I8,
        PrimitiveType::I16 => numerics::literals::LandedIntegerType::I16,
        PrimitiveType::I32 => numerics::literals::LandedIntegerType::I32,
        PrimitiveType::I64 => numerics::literals::LandedIntegerType::I64,
        PrimitiveType::U8 => numerics::literals::LandedIntegerType::U8,
        PrimitiveType::U16 => numerics::literals::LandedIntegerType::U16,
        PrimitiveType::U32 => numerics::literals::LandedIntegerType::U32,
        PrimitiveType::U64 => numerics::literals::LandedIntegerType::U64,
        _ => return None,
    };
    Some(checked_trees::CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "0",
        )
        .ok()?
        .with_landing(numerics::literals::IntegerLanding {
            landed_type,
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
        }),
    })
}

/// Existing storage transfers through the call's canonical source identity.
/// Copies and evaluated descriptors establish values; a whole-place transfer
/// must not acquire another result identity or another ownership event.
pub(super) fn argument_needs_establishment(kind: &CheckedStructuralValueKind) -> bool {
    let CheckedStructuralValueKind::Place(argument) = kind else {
        return true;
    };
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::CapturedStorage { .. }
        | CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ScalarParameter { .. }
        | CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralLocalBorrow { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
        | CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal { .. }
        | CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => false,
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
        | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. } => true,
    }
}
