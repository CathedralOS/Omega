//! Validate the selected physical ABI of the ordinary core descriptor. The
//! fixed late wire format is admitted only after actual field offsets, scalar
//! encodings and region stride match it; a coincidentally equal total size is
//! insufficient. Unsupported storage policies reject before emission.
use crate::structural_inputs::structural_reference_input as geometry;
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use terminal_psi::{
    StructuralFieldType, StructuralRecordStorage, StructuralTypeShape, find_structural_type,
};

pub(super) fn valid(source: &LegalizedScalarFunction, row: &LegalizedScalarInstruction) -> bool {
    let LegalizedScalarInstructionKind::EstablishNativeCodeLayout { result, shape, .. } = &row.kind
    else {
        return false;
    };
    let Some(signature) = &source.structural else {
        return false;
    };
    let types = &signature.structural_types;
    let Some(descriptor) = find_structural_type(types, result.structural_type) else {
        return false;
    };
    let StructuralTypeShape::Record {
        fields,
        storage: StructuralRecordStorage::Packed,
    } = &descriptor.shape
    else {
        return false;
    };
    if fields.len() != 2
        || fields
            .iter()
            .any(|field| field.relevance.is_erased() || field.storage.is_some())
        || *shape != ValueShape::integer(24, 8)
        || geometry::shape(result.structural_type, types) != Some(*shape)
    {
        return false;
    }
    let StructuralFieldType::Scalar(address) = fields[0].field_type else {
        return false;
    };
    if !matches!(address, semantic_vocabulary::ScalarType::Integer(ty) if ty.is_address() && ty.bits() == 64)
        || geometry::field_read(result.structural_type, &[], fields[0].id, address, types)
            != Some((0, 8))
    {
        return false;
    }
    let StructuralFieldType::Structural(view) = fields[1].field_type else {
        return false;
    };
    let Some(view_type) = find_structural_type(types, view) else {
        return false;
    };
    let StructuralTypeShape::ElementView { element, access } = view_type.shape else {
        return false;
    };
    if access.is_some_and(|access| access != terminal_psi::StructuralAccess::SharedBorrow)
        || geometry::shape(view, types) != Some(ValueShape::integer(16, 8))
    {
        return false;
    }
    // Packed record placement uses each exact field shape/alignment. There are
    // no authored offsets or codecs here: addr64 occupies 0..8 and the existing
    // ElementView base/extent descriptor ABI occupies the next aligned 16 bytes.
    let Some(view_shape) = geometry::field_storage_shape(&fields[1], types) else {
        return false;
    };
    let view_offset = (8u32 + u32::from(view_shape.alignment) - 1)
        / u32::from(view_shape.alignment)
        * u32::from(view_shape.alignment);
    if view_offset != 8 || view_shape != ValueShape::integer(16, 8) {
        return false;
    }
    let Some(region_type) = find_structural_type(types, element) else {
        return false;
    };
    let StructuralTypeShape::Record {
        fields: region,
        storage: StructuralRecordStorage::Packed,
    } = &region_type.shape
    else {
        return false;
    };
    if region.len() != 2
        || region
            .iter()
            .any(|field| field.relevance.is_erased() || field.storage.is_some())
        || geometry::shape(element, types) != Some(ValueShape::integer(16, 8))
    {
        return false;
    }
    let StructuralFieldType::Scalar(base) = region[0].field_type else {
        return false;
    };
    let StructuralFieldType::Scalar(length) = region[1].field_type else {
        return false;
    };
    base == address
        && matches!(length, semantic_vocabulary::ScalarType::Integer(ty) if !ty.is_address() && ty.bits() == 64 && ty.sign() == semantic_vocabulary::IntegerSign::Unsigned)
        && geometry::field_read(element, &[], region[0].id, base, types) == Some((0, 8))
        && geometry::field_read(element, &[], region[1].id, length, types) == Some((8, 8))
}
