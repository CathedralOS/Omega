//! Canonical scalar, structural, list, and optional-value carriers.
//!
//! These codecs are shared by the unit, proposition, structural-domain, and
//! operation identity encoders; they do not own any operation-family tags.

use super::{
    CanonicalBytes, IntegerSign, IntegerType, IntegerValue, PsiSemanticId, ScalarType,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, ValueBinding, structural_encoding::encode_structural_operation_result,
};
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
    result: abstract_operations::AbstractResult,
) {
    bytes.id(result.value);
    encode_scalar_type(bytes, result.scalar_type);
}

pub(super) fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
    crate::optimization_unit::encode_scalar_type(
        bytes,
        scalar_type,
        |bytes, tag| bytes.u8(tag),
        encode_integer_type,
    )
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
    bytes.id(parameter.structural_type);
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(bytes, &parameter.qualifications);
    bytes.slice(
        &parameter.projected_qualifications,
        |bytes, qualification| {
            bytes.slice(&qualification.path, encode_structural_path_segment);
            bytes.id(qualification.domain);
        },
    );
}

pub(super) fn encode_structural_binding(
    bytes: &mut CanonicalBytes,
    binding: &abstract_operations::AbstractStructuralBinding,
) {
    bytes.id(binding.parameter);
    encode_structural_argument(bytes, &binding.argument);
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
        StructuralPathSegment::AllElements => bytes.u8(10),
        StructuralPathSegment::Case(case) => {
            bytes.u8(7);
            bytes.u64(case.get());
        }
        StructuralPathSegment::Field(identity) => {
            bytes.u8(1);
            bytes.u32(identity.0);
        }
        StructuralPathSegment::FixedIndex(index) => {
            bytes.u8(2);
            bytes.u64(*index);
        }
        StructuralPathSegment::Referent => bytes.u8(3),
        StructuralPathSegment::FixedByteRange { start, end } => {
            bytes.u8(4);
            bytes.u64(*start);
            bytes.u64(*end);
        }
        StructuralPathSegment::RuntimeIndex { index, obligation } => {
            bytes.u8(5);
            bytes.u64(index.get());
            bytes.u64(obligation.get());
        }
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            bytes.u8(8);
            bytes.id(*structural_type);
            bytes.boolean(stored_scalar.is_some());
            if let Some(scalar) = stored_scalar {
                encode_scalar_type(bytes, *scalar);
            }
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

/// The window-root authority: a retained borrowed parameter row (tag 0) or an
/// owned completed home's producer and result row (tag 1).
pub(super) fn encode_window_root(
    bytes: &mut CanonicalBytes,
    root: &abstract_operations::StructuralWindowRoot,
) {
    match root {
        abstract_operations::StructuralWindowRoot::Parameter(parameter) => {
            bytes.u8(0);
            encode_structural_parameter(bytes, parameter);
        }
        abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => {
            bytes.u8(1);
            bytes.id(*producer);
            encode_structural_operation_result(bytes, result);
        }
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            bytes.u8(2);
            bytes.id(*block);
            encode_structural_parameter(bytes, parameter);
        }
    }
}
