//! Optimizer module role: executable entrance. Canonical abstract-operation encoding entrance.
//!
//! This exhaustive routing match owns the stable family partition. Each leaf
//! owns exact variant tags and field order; `scalar_shapes` owns repeated
//! scalar-operation fields. Identity-wide carriers live beside this entrance.

use super::{AbstractOperation, CanonicalBytes};
mod atomic;
mod calls_and_effects;
mod control;
mod ieee_float;
mod scalar;
mod scalar_shapes;
mod structural;

pub(super) fn encode_operation(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::ByteSequenceSubslice { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewSubslice { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishStructuralArray { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | abstract_operations::move_or_store_structural_field_pattern!()
        | O::StructuralLeafCopy { .. }
        | O::EstablishRecord { .. } => structural::encode(bytes, operation),

        O::AtomicEvent { .. } => atomic::encode(bytes, operation),

        O::CallStructural { .. }
        | O::SelectImplementation { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. } => calls_and_effects::encode(bytes, operation),

        O::IeeeFloatConstant { .. }
        | O::FloatResultPolicy { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. } => ieee_float::encode(bytes, operation),

        O::IntegerConstant { .. }
        | O::BooleanConstant { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequencePredicate { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequenceWrite { .. }
        | abstract_operations::byte_sequence_store_pair_pattern!( .. )
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceLength { .. }
        | O::ElementViewLength { .. }
        | abstract_operations::element_view_read_write_pair_pattern!(..)
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::ScalarRepresentationView { .. }
        | O::IntegerBitwiseAnd { .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | O::IntegerShift { .. }
        | O::IntegerArithmetic { .. }
        | O::TrappingInteger { .. } => scalar::encode(bytes, operation),

        abstract_operations::jump_or_conditional_pattern!()
        | O::StructuralCase { .. }
        | abstract_operations::return_or_return_unit_pattern!()
        | O::ReturnStructural { .. }
        | O::ExternalCompletion { .. }
        | O::Crash { .. } => control::encode(bytes, operation),
    }
}

/// An optional field identity: the field's nonzero identity, or zero (which
/// no field takes) when the operation names its whole projected place.
fn encode_optional_field(
    bytes: &mut CanonicalBytes,
    field: Option<semantic_vocabulary::StructuralFieldId>,
) {
    bytes.u64(field.map_or(0, semantic_vocabulary::StructuralFieldId::get));
}
