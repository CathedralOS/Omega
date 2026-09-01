//! Canonical scalar, structural, list, and optional-value carriers.
//!
//! These codecs are shared by the unit, proposition, structural-domain, and
//! operation identity encoders; they do not own any operation-family tags.

use super::*;

pub(super) fn encode_binding(bytes: &mut CanonicalBytes, binding: &ValueBinding) {
    bytes.id(binding.parameter);
    bytes.id(binding.argument);
    encode_scalar_type(bytes, binding.scalar_type);
}

pub(super) fn encode_optional<T>(
    bytes: &mut CanonicalBytes,
    value: Option<&T>,
    encode: impl Fn(&mut CanonicalBytes, &T),
) {
    bytes.boolean(value.is_some());
    if let Some(value) = value {
        encode(bytes, value);
    }
}

pub(super) fn encode_ids<T: PsiSemanticId>(bytes: &mut CanonicalBytes, ids: &[T]) {
    bytes.len(ids.len());
    for id in ids {
        bytes.id(*id);
    }
}

pub(super) fn encode_abstract_result(
    bytes: &mut CanonicalBytes,
    result: omega_abstract_operations::AbstractResult,
) {
    bytes.id(result.value);
    encode_scalar_type(bytes, result.scalar_type);
}

pub(super) fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.u8(1),
        ScalarType::Integer(integer) => {
            bytes.u8(2);
            encode_integer_type(bytes, integer);
        }
        ScalarType::IeeeFloat(format) => {
            bytes.u8(3);
            bytes.u8(match format {
                psi_core::IeeeFloatFormat::Binary32 => 1,
                psi_core::IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut CanonicalBytes, integer_type: IntegerType) {
    bytes.u8(match integer_type.sign() {
        IntegerSign::Unsigned => 1,
        IntegerSign::Signed => 2,
    });
    bytes.u16(integer_type.bits());
}

pub(super) fn encode_integer_value(bytes: &mut CanonicalBytes, value: IntegerValue) {
    match value {
        IntegerValue::Unsigned(value) => {
            bytes.u8(1);
            bytes.u128(value);
        }
        IntegerValue::Signed(value) => {
            bytes.u8(2);
            bytes.bytes(&value.to_le_bytes());
        }
    }
}

pub(super) fn encode_structural_parameter(
    bytes: &mut CanonicalBytes,
    parameter: &StructuralParameterDeclaration,
) {
    bytes.id(parameter.place);
    bytes.u32(parameter.position);
    bytes.boolean(parameter.is_self);
    bytes.id(parameter.structural_type);
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(bytes, &parameter.qualifications);
}

pub(super) fn encode_structural_argument(
    bytes: &mut CanonicalBytes,
    argument: &StructuralArgument,
) {
    bytes.id(argument.place);
    bytes.slice(&argument.path, encode_structural_path_segment);
    encode_access(bytes, argument.access);
}

pub(super) fn encode_structural_path_segment(
    bytes: &mut CanonicalBytes,
    segment: &StructuralPathSegment,
) {
    match segment {
        StructuralPathSegment::Field(identity) => {
            bytes.u8(1);
            bytes.string(identity);
        }
        StructuralPathSegment::FixedIndex(index) => {
            bytes.u8(2);
            bytes.u64(*index);
        }
    }
}

pub(super) fn encode_access(bytes: &mut CanonicalBytes, access: StructuralAccess) {
    bytes.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(super) fn encode_multiplicity(
    bytes: &mut CanonicalBytes,
    multiplicity: StructuralMultiplicity,
) {
    bytes.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}
