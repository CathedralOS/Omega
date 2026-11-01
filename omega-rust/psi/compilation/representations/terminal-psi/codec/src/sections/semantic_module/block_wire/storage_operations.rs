//! Storage operations whose tag depends on their contents: a reference with
//! or without a storage view, pathless or projected reads, releases and
//! primitive stores, and the store disposition. Fixed-tag storage operations
//! are rows of `operation_wire!`.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::wire::{Reader, Writer};
use super::super::wire_fields::{Argument, Field, OptionalField};
use super::operation_tags;
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use semantic_vocabulary::CanonicalStructuralPathSegment;
use semantic_vocabulary::{ObligationId, PlaceId, StructuralFieldId, ValueId};
use terminal_psi::OperationKind;
use terminal_psi::ReferenceStorageView;
use terminal_psi::{StructuralArgument, StructuralPathSegment};

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
    OptionalField::write(writer, &field)?;
    Argument::write(writer, &value)
}

pub(super) fn decode_store_structural_field(
    reader: &mut Reader<'_>,
    disposition: terminal_psi::StructuralStoreDisposition,
) -> Result<OperationKind, CodecError> {
    Ok(OperationKind::StoreStructuralField {
        disposition,
        destination: reader.id("PlaceId")?,
        path: decode_structural_path(reader)?,
        field: OptionalField::read(reader)?,
        value: Argument::read(reader)?,
    })
}
