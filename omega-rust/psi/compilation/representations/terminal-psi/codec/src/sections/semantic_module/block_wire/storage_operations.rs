//! Operations that read or write retained storage on the wire: references,
//! primitive reads and stores, byte sequences and byte-sequence fields,
//! structural scalar and field reads and stores, and case membership.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use super::{decode_scalar_field_path, encode_scalar_field_path};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use semantic_vocabulary::CanonicalStructuralPathSegment;
use semantic_vocabulary::{ObligationId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId};
use terminal_psi::OperationKind;
use terminal_psi::ReferenceStorageView;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

pub(super) fn encode_establish_placed_reference(
    writer: &mut Writer,
    source: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_PLACED_REFERENCE);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "placed reference source path", &source.path)
}

pub(super) fn decode_establish_placed_reference(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishPlacedReference {
        source: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}

pub(super) fn encode_establish_reference(
    writer: &mut Writer,
    source: StructuralArgument,
    storage: Option<ReferenceStorageView>,
) -> Result<(), CodecError> {
    writer.u8(if storage.is_some() {
        operation_tags::ESTABLISH_STORAGE_VIEW_REFERENCE
    } else {
        operation_tags::ESTABLISH_REFERENCE
    });
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "reference source path", &source.path)?;
    if let Some(storage) = storage {
        writer.id(storage.byte_offset);
        writer.boolean(storage.stored_scalar.is_some());
        if let Some(stored_scalar) = storage.stored_scalar {
            encode_scalar_type(writer, stored_scalar);
        }
    }
    Ok(())
}

pub(super) fn decode_establish_reference(
    reader: &mut Reader<'_>,
    has_storage_view: bool,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishReference {
        source: terminal_psi::StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
        storage: if has_storage_view {
            Some(ReferenceStorageView {
                byte_offset: reader.id("ValueId")?,
                stored_scalar: if reader.boolean()? {
                    Some(decode_scalar_type(reader)?)
                } else {
                    None
                },
            })
        } else {
            None
        },
    })
}

pub(super) fn encode_release_reference(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
) -> Result<(), CodecError> {
    writer.u8(if path.is_empty() {
        operation_tags::RELEASE_REFERENCE
    } else {
        operation_tags::PROJECTED_RELEASE_REFERENCE
    });
    writer.id(source);
    if !path.is_empty() {
        encode_structural_path(writer, "reference release path", &path)?;
    }
    Ok(())
}

pub(super) fn decode_release_reference(
    reader: &mut Reader<'_>,
    projected: bool,
) -> Result<OperationKind, CodecError> {
    let source = reader.id("PlaceId")?;
    let path = if projected {
        decode_structural_path(reader)?
    } else {
        Vec::new()
    };
    if projected && path.is_empty() {
        return crate::codec_error::malformed("projected release requires a nonempty path");
    }
    Ok(OperationKind::ReleaseReference { source, path })
}

pub(super) fn encode_primitive_scalar_read(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
) -> Result<(), CodecError> {
    if path.is_empty() {
        writer.u8(operation_tags::PRIMITIVE_SCALAR_READ);
        writer.id(source);
    } else {
        writer.u8(operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ);
        writer.id(source);
        encode_structural_path(writer, "primitive source path", &path)?;
    }
    Ok(())
}

pub(super) fn decode_primitive_scalar_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::PrimitiveScalarRead {
        path: Vec::new(),
        source: reader.id("PlaceId")?,
    })
}

pub(super) fn decode_projected_primitive_scalar_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let source = reader.id("PlaceId")?;
    let path = decode_structural_path(reader)?;
    if path.is_empty() {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty projected primitive path",
        ));
    }
    Ok(OperationKind::PrimitiveScalarRead { source, path })
}

pub(super) fn encode_structural_case_membership(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    case: StructuralCaseId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_CASE_MEMBERSHIP);
    writer.id(source);
    encode_structural_path(writer, "case membership path", &path)?;
    writer.id(case);
    Ok(())
}

pub(super) fn decode_structural_case_membership(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralCaseMembership {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        case: reader.id("StructuralCaseId")?,
    })
}

pub(super) fn encode_structural_leaf_copy(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_LEAF_COPY);
    writer.id(source);
    encode_structural_path(writer, "leaf copy path", &path)?;
    Ok(())
}

pub(super) fn decode_structural_leaf_copy(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralLeafCopy {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
    })
}

pub(super) fn encode_structural_case_leaf_copy(
    writer: &mut Writer,
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_CASE_LEAF_COPY);
    super::super::structural_field_wire::encode_canonical_structural_field(
        writer,
        source,
        path,
        "case leaf copy path",
    )
}

pub(super) fn decode_structural_case_leaf_copy(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let (source, path) =
        super::super::structural_field_wire::decode_canonical_structural_field(reader)?;
    Ok(OperationKind::StructuralCaseLeafCopy { source, path })
}

pub(super) fn encode_byte_sequence_subslice(
    writer: &mut Writer,
    source: StructuralArgument,
    start: ValueId,
    end: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_SUBSLICE);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "descriptor source path", &source.path)?;
    writer.id(start);
    writer.id(end);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_byte_sequence_subslice(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceSubslice {
        source: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
        start: reader.id("ValueId")?,
        end: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_byte_sequence_write(
    writer: &mut Writer,
    destination: PlaceId,
    index: ValueId,
    value: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_WRITE);
    writer.id(destination);
    writer.id(index);
    writer.id(value);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_byte_sequence_write(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceWrite {
        destination: reader.id("PlaceId")?,
        index: reader.id("ValueId")?,
        value: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_element_view_write(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    index: ValueId,
    value: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ELEMENT_VIEW_WRITE);
    writer.id(destination);
    encode_structural_path(writer, "element write path", &path)?;
    writer.id(index);
    writer.id(value);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_element_view_write(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ElementViewWrite {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        index: reader.id("ValueId")?,
        value: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_byte_sequence_read(
    writer: &mut Writer,
    source: PlaceId,
    index: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_READ);
    writer.id(source);
    writer.id(index);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_byte_sequence_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceRead {
        source: reader.id("PlaceId")?,
        index: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_byte_sequence_length(
    writer: &mut Writer,
    source: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_LENGTH);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "descriptor source path", &source.path)?;
    Ok(())
}

pub(super) fn decode_byte_sequence_length(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceLength {
        source: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}

pub(super) fn encode_establish_element_view(
    writer: &mut Writer,
    destination: PlaceId,
    source: StructuralArgument,
    element: semantic_vocabulary::StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_ELEMENT_VIEW);
    writer.id(destination);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "element view source path", &source.path)?;
    writer.id(element);
    encode_element_view_region(writer, region);
    Ok(())
}

pub(super) fn decode_establish_element_view(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishElementView {
        destination: reader.id("PlaceId")?,
        source: terminal_psi::StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
        element: reader.id("StructuralTypeId")?,
        region: decode_element_view_region(reader)?,
    })
}

/// A mutable element view carries the same wire shape as its shared sibling;
/// the tag alone selects which loan the establishment grants.
pub(super) fn encode_establish_mutable_element_view(
    writer: &mut Writer,
    destination: PlaceId,
    source: StructuralArgument,
    element: semantic_vocabulary::StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_MUTABLE_ELEMENT_VIEW);
    writer.id(destination);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "mutable element view source path", &source.path)?;
    writer.id(element);
    encode_element_view_region(writer, region);
    Ok(())
}

pub(super) fn decode_establish_mutable_element_view(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishMutableElementView {
        destination: reader.id("PlaceId")?,
        source: terminal_psi::StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
        element: reader.id("StructuralTypeId")?,
        region: decode_element_view_region(reader)?,
    })
}

pub(super) fn encode_element_view_length(
    writer: &mut Writer,
    source: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ELEMENT_VIEW_LENGTH);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "element descriptor path", &source.path)?;
    Ok(())
}

pub(super) fn decode_element_view_length(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ElementViewLength {
        source: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}

/// A scalar element keeps the pathless spelling; a record element's field
/// read carries its element path under its own tag, as a projected primitive
/// read does, so each read has exactly one wire form.
pub(super) fn encode_element_view_read(
    writer: &mut Writer,
    source: PlaceId,
    index: ValueId,
    length: ValueId,
    obligation: ObligationId,
    path: Vec<StructuralPathSegment>,
) -> Result<(), CodecError> {
    writer.u8(if path.is_empty() {
        operation_tags::ELEMENT_VIEW_READ
    } else {
        operation_tags::PROJECTED_ELEMENT_VIEW_READ
    });
    writer.id(source);
    writer.id(index);
    writer.id(length);
    writer.id(obligation);
    if !path.is_empty() {
        encode_structural_path(writer, "element view read path", &path)?;
    }
    Ok(())
}

pub(super) fn decode_element_view_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ElementViewRead {
        source: reader.id("PlaceId")?,
        index: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
        path: Vec::new(),
    })
}

pub(super) fn decode_projected_element_view_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let source = reader.id("PlaceId")?;
    let index = reader.id("ValueId")?;
    let length = reader.id("ValueId")?;
    let obligation = reader.id("ObligationId")?;
    let path = decode_structural_path(reader)?;
    if path.is_empty() {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty projected element view read path",
        ));
    }
    Ok(OperationKind::ElementViewRead {
        source,
        index,
        length,
        obligation,
        path,
    })
}

pub(super) fn encode_element_view_subslice(
    writer: &mut Writer,
    source: StructuralArgument,
    start: ValueId,
    end: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ELEMENT_VIEW_SUBSLICE);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "descriptor source path", &source.path)?;
    writer.id(start);
    writer.id(end);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_element_view_subslice(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ElementViewSubslice {
        source: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
        start: reader.id("ValueId")?,
        end: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_write_only_primitive_store(
    writer: &mut Writer,
    destination: PlaceId,
    value: ValueId,
    path: Vec<StructuralPathSegment>,
    range_obligation: Option<ObligationId>,
) -> Result<(), CodecError> {
    if let Some(obligation) = range_obligation {
        writer.u8(operation_tags::RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE);
        writer.id(destination);
        encode_structural_path(writer, "restricted primitive destination path", &path)?;
        writer.id(value);
        writer.id(obligation);
        return Ok(());
    }
    if path.is_empty() {
        writer.u8(operation_tags::WRITE_ONLY_PRIMITIVE_STORE);
        writer.id(destination);
    } else {
        writer.u8(operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE);
        writer.id(destination);
        encode_structural_path(writer, "primitive destination path", &path)?;
    }
    writer.id(value);
    Ok(())
}

pub(super) fn decode_projected_write_only_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let destination = reader.id("PlaceId")?;
    let path = decode_structural_path(reader)?;
    if path.is_empty() {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty projected primitive path",
        ));
    }
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination,
        path,
        value: reader.id("ValueId")?,
        range_obligation: None,
    })
}

pub(super) fn decode_restricted_write_only_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        value: reader.id("ValueId")?,
        range_obligation: Some(reader.id("ObligationId")?),
    })
}

pub(super) fn decode_write_only_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        path: Vec::new(),
        destination: reader.id("PlaceId")?,
        value: reader.id("ValueId")?,
        range_obligation: None,
    })
}

pub(super) fn encode_structural_byte_sequence_field_store(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    source: PlaceId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE);
    writer.id(destination);
    encode_structural_path(writer, "structural byte sequence field store path", &path)?;
    writer.id(field);
    writer.id(source);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_structural_byte_sequence_field_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralByteSequenceFieldStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        source: reader.id("PlaceId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_structural_byte_view_field_store(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: Option<StructuralFieldId>,
    source: PlaceId,
    predicate_obligation: Option<ObligationId>,
) -> Result<(), CodecError> {
    writer.u8(if predicate_obligation.is_some() {
        operation_tags::PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE
    } else {
        operation_tags::STRUCTURAL_BYTE_VIEW_FIELD_STORE
    });
    writer.id(destination);
    encode_structural_path(writer, "structural byte view field store path", &path)?;
    encode_optional_field(writer, field);
    writer.id(source);
    if let Some(obligation) = predicate_obligation {
        writer.id(obligation);
    }
    Ok(())
}

pub(super) fn decode_structural_byte_view_field_store(
    reader: &mut Reader<'_>,
    predicate_checked: bool,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralByteViewFieldStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: decode_optional_field(reader)?,
        source: reader.id("PlaceId")?,
        predicate_obligation: if predicate_checked {
            Some(reader.id("ObligationId")?)
        } else {
            None
        },
    })
}

pub(super) fn encode_structural_byte_sequence_field_length(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH);
    writer.id(source);
    encode_structural_path(writer, "structural byte sequence field length path", &path)?;
    writer.id(field);
    Ok(())
}

pub(super) fn decode_structural_byte_sequence_field_length(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralByteSequenceFieldLength {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
    })
}

pub(super) fn encode_structural_byte_sequence_field_byte_store(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    index: ValueId,
    value: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE);
    writer.id(destination);
    encode_structural_path(
        writer,
        "structural byte sequence field byte store path",
        &path,
    )?;
    writer.id(field);
    writer.id(index);
    writer.id(value);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_structural_byte_sequence_field_byte_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralByteSequenceFieldByteStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        index: reader.id("ValueId")?,
        value: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_structural_byte_sequence_field_read(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    index: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_READ);
    writer.id(source);
    encode_structural_path(writer, "structural byte sequence field read path", &path)?;
    writer.id(field);
    writer.id(index);
    writer.id(length);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_structural_byte_sequence_field_read(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralByteSequenceFieldRead {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        index: reader.id("ValueId")?,
        length: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
    })
}

pub(super) fn encode_structural_scalar_field_store(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    value: ValueId,
    range_obligation: Option<ObligationId>,
) -> Result<(), CodecError> {
    writer.u8(if range_obligation.is_some() {
        operation_tags::RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE
    } else {
        operation_tags::STRUCTURAL_SCALAR_FIELD_STORE
    });
    writer.id(destination);
    encode_structural_path(writer, "structural scalar field store path", &path)?;
    writer.id(field);
    writer.id(value);
    if let Some(obligation) = range_obligation {
        writer.id(obligation);
    }
    Ok(())
}

pub(super) fn decode_structural_scalar_field_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralScalarFieldStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        value: reader.id("ValueId")?,
        range_obligation: None,
    })
}

pub(super) fn decode_range_checked_structural_scalar_field_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StructuralScalarFieldStore {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        value: reader.id("ValueId")?,
        range_obligation: Some(reader.id("ObligationId")?),
    })
}

pub(super) fn encode_boolean_structural_field(
    writer: &mut Writer,
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BOOLEAN_STRUCTURAL_FIELD);
    writer.id(source);
    encode_scalar_field_path(writer, path)?;
    writer.id(field);
    Ok(())
}

pub(super) fn decode_boolean_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::BooleanStructuralField {
        source: reader.id("PlaceId")?,
        path: decode_scalar_field_path(reader)?,
        field: reader.id("StructuralFieldId")?,
    })
}

pub(super) fn encode_scalar_structural_field(
    writer: &mut Writer,
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::SCALAR_STRUCTURAL_FIELD);
    writer.id(source);
    encode_scalar_field_path(writer, path)?;
    writer.id(field);
    Ok(())
}

pub(super) fn decode_scalar_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ScalarStructuralField {
        source: reader.id("PlaceId")?,
        path: decode_scalar_field_path(reader)?,
        field: reader.id("StructuralFieldId")?,
    })
}

/// An optional field identity on the wire: the declared field's nonzero
/// identity, or zero when the operation names its whole projected place (a
/// root window that vacates the whole referent, or a whole descriptor slot).
/// Nonzero identities keep their previous spelling, so every field form
/// encodes exactly as before; the whole form is the one value a field
/// identity never takes.
fn encode_optional_field(writer: &mut Writer, field: Option<StructuralFieldId>) {
    writer.u64(field.map_or(0, StructuralFieldId::get));
}

fn decode_optional_field(reader: &mut Reader<'_>) -> Result<Option<StructuralFieldId>, CodecError> {
    Ok(StructuralFieldId::new(reader.u64()?))
}

pub(super) fn encode_move_structural_field(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: Option<StructuralFieldId>,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::MOVE_STRUCTURAL_FIELD);
    writer.id(source);
    encode_structural_path(writer, "structural field move path", &path)?;
    encode_optional_field(writer, field);
    Ok(())
}

pub(super) fn decode_move_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::MoveStructuralField {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: decode_optional_field(reader)?,
    })
}

pub(super) fn encode_store_structural_field(
    writer: &mut Writer,
    destination: PlaceId,
    disposition: terminal_psi::StructuralStoreDisposition,
    path: Vec<StructuralPathSegment>,
    field: Option<StructuralFieldId>,
    value: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(match disposition {
        terminal_psi::StructuralStoreDisposition::Vacant => operation_tags::STORE_STRUCTURAL_FIELD,
        terminal_psi::StructuralStoreDisposition::ReplaceDiscardable => {
            operation_tags::REPLACE_DISCARDABLE_STRUCTURAL_FIELD
        }
    });
    writer.id(destination);
    encode_structural_path(writer, "structural field store path", &path)?;
    encode_optional_field(writer, field);
    writer.id(value.place);
    super::super::structural_signature_wire::encode_structural_access(writer, value.access);
    encode_structural_path(writer, "structural field store value path", &value.path)?;
    Ok(())
}

pub(super) fn decode_store_structural_field(
    reader: &mut Reader<'_>,
    disposition: terminal_psi::StructuralStoreDisposition,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StoreStructuralField {
        disposition,
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: decode_optional_field(reader)?,
        value: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}

fn encode_element_view_region(writer: &mut Writer, region: terminal_psi::ElementViewRegion) {
    match region {
        terminal_psi::ElementViewRegion::WholeCollection => writer.u8(0),
        terminal_psi::ElementViewRegion::RepresentationBytes {
            start,
            end,
            obligation,
        } => {
            writer.u8(1);
            writer.id(start);
            writer.id(end);
            writer.id(obligation);
        }
    }
}
fn decode_element_view_region(
    reader: &mut Reader<'_>,
) -> Result<terminal_psi::ElementViewRegion, CodecError> {
    match reader.u8()? {
        0 => Ok(terminal_psi::ElementViewRegion::WholeCollection),
        1 => Ok(terminal_psi::ElementViewRegion::RepresentationBytes {
            start: reader.id("ValueId")?,
            end: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
        }),
        tag => Err(CodecError::InvalidTag("element view region", tag)),
    }
}
