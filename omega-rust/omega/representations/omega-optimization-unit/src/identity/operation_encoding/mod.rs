//! Optimizer module role: executable entrance. Canonical abstract-operation encoding entrance.
//!
//! This exhaustive routing match owns the stable family partition. Each leaf
//! owns exact variant tags and field order; `scalar_shapes` owns repeated
//! scalar-operation fields. Identity-wide carriers live beside this entrance.

use super::*;

mod calls_and_effects;
mod control;
mod scalar;
mod scalar_shapes;
mod structural;

pub(super) fn encode_operation(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::EstablishPayloadlessCase { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishAffineScalarRecord { .. } => structural::encode(bytes, operation),

        O::DynamicDescriptorParameter { .. }
        | O::StoreDynamicDescriptor { .. }
        | O::CallUnit { .. }
        | O::CallUnitWithDynamicArguments { .. }
        | O::CallStructuralScalar { .. }
        | O::CallStructuralScalarWithDynamicArguments { .. }
        | O::CallDynamicScalar { .. }
        | O::CallStoredDynamicScalar { .. }
        | O::CallDynamicParameterScalar { .. }
        | O::CallDynamicUnit { .. }
        | O::CallDynamicParameterUnit { .. }
        | O::CallStructural { .. }
        | O::BoundaryCall { .. }
        | O::PortWrite { .. }
        | O::Call { .. } => calls_and_effects::encode(bytes, operation),

        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::BooleanConstant { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::ExactIntegerDivide { .. }
        | O::ExactIntegerRemainder { .. }
        | O::WrappingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::SaturatingIntegerMultiply { .. } => scalar::encode(bytes, operation),

        O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::Crash { .. } => control::encode(bytes, operation),
    }
}
