//! Operations that read or write retained storage on the wire: references,
//! primitive reads and stores, byte sequences and byte-sequence fields,
//! structural scalar and field reads and stores, case membership and
//! dynamic descriptors.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use super::operation_tags;
use super::{decode_scalar_field_path, encode_scalar_field_path};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use semantic_vocabulary::CanonicalStructuralPathSegment;
use semantic_vocabulary::{ObligationId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId};
use terminal_psi::OperationKind;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

pub(super) fn encode_establish_reference(
    writer: &mut Writer,
    source: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::ESTABLISH_REFERENCE);
    writer.id(source.place);
    super::super::structural_signature_wire::encode_structural_access(writer, source.access);
    encode_structural_path(writer, "reference source path", &source.path)?;
    Ok(())
}

pub(super) fn decode_establish_reference(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::EstablishReference {
        source: terminal_psi::StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}

pub(super) fn encode_release_reference(
    writer: &mut Writer,
    source: PlaceId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::RELEASE_REFERENCE);
    writer.id(source);
    Ok(())
}

pub(super) fn decode_release_reference(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ReleaseReference {
        source: reader.id("PlaceId")?,
    })
}

pub(super) fn encode_primitive_scalar_read(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
) -> Result<(), CodecError> {
    if path.is_empty() {
        writer.u8(operation_tags::PRIMITIVE_SCALAR_READ);
        writer.id(source);
    } else {
        writer.u8(operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ);
        super::super::structural_field_wire::encode_canonical_structural_field(
            writer,
            source,
            &path,
            "primitive source path",
        )?;
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
    let (source, path) =
        super::super::structural_field_wire::decode_canonical_structural_field(reader)?;
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

pub(super) fn encode_byte_sequence_subslice(
    writer: &mut Writer,
    source: PlaceId,
    start: ValueId,
    end: ValueId,
    length: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_SUBSLICE);
    writer.id(source);
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
        source: reader.id("PlaceId")?,
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
    source: PlaceId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::BYTE_SEQUENCE_LENGTH);
    writer.id(source);
    Ok(())
}

pub(super) fn decode_byte_sequence_length(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::ByteSequenceLength {
        source: reader.id("PlaceId")?,
    })
}

pub(super) fn encode_write_only_primitive_store(
    writer: &mut Writer,
    destination: PlaceId,
    value: ValueId,
    path: Vec<CanonicalStructuralPathSegment>,
) -> Result<(), CodecError> {
    if path.is_empty() {
        writer.u8(operation_tags::WRITE_ONLY_PRIMITIVE_STORE);
        writer.id(destination);
    } else {
        writer.u8(operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE);
        super::super::structural_field_wire::encode_canonical_structural_field(
            writer,
            destination,
            &path,
            "primitive destination path",
        )?;
    }
    writer.id(value);
    Ok(())
}

pub(super) fn decode_projected_write_only_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let (destination, path) =
        super::super::structural_field_wire::decode_canonical_structural_field(reader)?;
    if path.is_empty() {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty projected primitive path",
        ));
    }
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        destination,
        path,
        value: reader.id("ValueId")?,
    })
}

pub(super) fn decode_write_only_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::WriteOnlyPrimitiveStore {
        path: Vec::new(),
        destination: reader.id("PlaceId")?,
        value: reader.id("ValueId")?,
    })
}

pub(super) fn encode_write_only_indexed_primitive_store(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    index: ValueId,
    value: ValueId,
    obligation: ObligationId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::WRITE_ONLY_INDEXED_PRIMITIVE_STORE);
    super::super::structural_field_wire::encode_canonical_structural_field(
        writer,
        destination,
        &path,
        "indexed primitive destination path",
    )?;
    writer.id(index);
    writer.id(value);
    writer.id(obligation);
    Ok(())
}

pub(super) fn decode_write_only_indexed_primitive_store(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    let (destination, path) =
        super::super::structural_field_wire::decode_canonical_structural_field(reader)?;
    Ok(OperationKind::WriteOnlyIndexedPrimitiveStore {
        destination,
        path,
        index: reader.id("ValueId")?,
        value: reader.id("ValueId")?,
        obligation: reader.id("ObligationId")?,
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

pub(super) fn encode_store_dynamic_descriptor(
    writer: &mut Writer,
    descriptor_ordinal: u32,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STORE_DYNAMIC_DESCRIPTOR);
    writer.u32(descriptor_ordinal);
    Ok(())
}

pub(super) fn decode_store_dynamic_descriptor(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StoreDynamicDescriptor {
        descriptor_ordinal: reader.u32()?,
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

pub(super) fn encode_integer_structural_field(
    writer: &mut Writer,
    source: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::INTEGER_STRUCTURAL_FIELD);
    writer.id(source);
    encode_scalar_field_path(writer, path)?;
    writer.id(field);
    Ok(())
}

pub(super) fn decode_integer_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::IntegerStructuralField {
        source: reader.id("PlaceId")?,
        path: decode_scalar_field_path(reader)?,
        field: reader.id("StructuralFieldId")?,
    })
}

pub(super) fn encode_move_structural_field(
    writer: &mut Writer,
    source: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::MOVE_STRUCTURAL_FIELD);
    writer.id(source);
    encode_structural_path(writer, "structural field move path", &path)?;
    writer.id(field);
    Ok(())
}

pub(super) fn decode_move_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::MoveStructuralField {
        source: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
    })
}

pub(super) fn encode_store_structural_field(
    writer: &mut Writer,
    destination: PlaceId,
    path: Vec<StructuralPathSegment>,
    field: StructuralFieldId,
    value: StructuralArgument,
) -> Result<(), CodecError> {
    writer.u8(operation_tags::STORE_STRUCTURAL_FIELD);
    writer.id(destination);
    encode_structural_path(writer, "structural field store path", &path)?;
    writer.id(field);
    writer.id(value.place);
    super::super::structural_signature_wire::encode_structural_access(writer, value.access);
    encode_structural_path(writer, "structural field store value path", &value.path)?;
    Ok(())
}

pub(super) fn decode_store_structural_field(
    reader: &mut Reader<'_>,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StoreStructuralField {
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: reader.id("StructuralFieldId")?,
        value: StructuralArgument {
            place: reader.id("PlaceId")?,
            access: super::super::structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        },
    })
}
