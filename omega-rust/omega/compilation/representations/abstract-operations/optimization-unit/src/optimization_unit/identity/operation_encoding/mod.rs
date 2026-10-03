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
#[cfg(test)]
mod integer_policy_tags_tests;
mod scalar;
mod scalar_shapes;
mod structural;
#[cfg(test)]
mod structural_scalar_field_read_tags_tests;

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
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::MoveStructuralField { .. }
        | O::StoreStructuralField { .. }
        | O::StructuralLeafCopy { .. }
        | O::EstablishRecord { .. } => structural::encode(bytes, operation),

        O::AtomicEvent { .. } => atomic::encode(bytes, operation),

        O::DynamicDescriptorParameter { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::CallStructural { .. }
        | O::CallDynamic { .. }
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
        | O::ByteSequenceStore { .. }
        | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceLength { .. }
        | O::ElementViewLength { .. }
        | O::ElementViewRead { .. }
        | O::ElementViewWrite { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::StructuralByteSequenceFieldRead { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::ScalarRepresentationView { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerShift { .. }
        | O::IntegerArithmetic { .. }
        | O::TrappingInteger { .. } => scalar::encode(bytes, operation),

        O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => control::encode(bytes, operation),
    }
}
