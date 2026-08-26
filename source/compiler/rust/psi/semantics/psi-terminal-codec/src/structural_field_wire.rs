//! Canonical structural-field wire primitives.
//!
//! This module owns exact field-kind, relevance, float, byte-sequence, and
//! canonical structural-path tags. Recursive structural-type framing remains
//! in the parent codec.

use psi_core::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatComparisonKind,
    IeeeFloatFormat, IeeeFloatStructuralField, PlaceId,
};
use psi_terminal::{
    BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
};

use super::CodecError;
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::wire::{Reader, Writer};

pub(super) fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

pub(super) fn encode_ieee_float_comparison_kind(
    writer: &mut Writer,
    kind: IeeeFloatComparisonKind,
) {
    writer.u8(match kind {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
    });
}

pub(super) fn decode_ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatComparisonKind, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatComparisonKind::Equal),
        2 => Ok(IeeeFloatComparisonKind::NotEqual),
        tag => Err(CodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
    }
}

pub(super) fn decode_ieee_float_format(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatFormat, CodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

pub(super) fn encode_ieee_float_field(
    writer: &mut Writer,
    field: &IeeeFloatStructuralField,
) -> Result<(), CodecError> {
    encode_canonical_structural_field(writer, field.root(), field.path(), "IEEE float field path")
}

pub(super) fn decode_ieee_float_field(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatStructuralField, CodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    IeeeFloatStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
}

pub(super) fn encode_byte_sequence_field(
    writer: &mut Writer,
    field: &ByteSequenceStructuralField,
) -> Result<(), CodecError> {
    encode_canonical_structural_field(
        writer,
        field.root(),
        field.path(),
        "byte-sequence field path",
    )
}

pub(super) fn decode_byte_sequence_field(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceStructuralField, CodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    ByteSequenceStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
}

pub(super) fn encode_canonical_structural_field(
    writer: &mut Writer,
    root: PlaceId,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), CodecError> {
    writer.id(root);
    writer.len(length_label, path.len())?;
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                writer.u8(1);
                writer.id(*field);
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
            CanonicalStructuralPathSegment::Case(case) => {
                writer.u8(3);
                writer.id(*case);
            }
        }
    }
    Ok(())
}

pub(super) fn decode_canonical_structural_field(
    reader: &mut Reader<'_>,
) -> Result<(PlaceId, Vec<CanonicalStructuralPathSegment>), CodecError> {
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut path = Vec::with_capacity(count as usize);
    for _ in 0..count {
        path.push(match reader.u8()? {
            1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
            2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
            3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
            tag => {
                return Err(CodecError::InvalidTag(
                    "CanonicalStructuralPathSegment",
                    tag,
                ));
            }
        });
    }
    Ok((root, path))
}

pub(super) fn encode_byte_sequence_carrier(writer: &mut Writer, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => writer.u8(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            writer.u8(2);
            writer.u64(capacity);
        }
    }
}

pub(super) fn decode_byte_sequence_carrier(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceCarrier, CodecError> {
    match reader.u8()? {
        1 => Ok(ByteSequenceCarrier::BorrowedView),
        2 => Ok(ByteSequenceCarrier::BoundedOwned {
            capacity: reader.u64()?,
        }),
        tag => Err(CodecError::InvalidTag("ByteSequenceCarrier", tag)),
    }
}

pub(super) fn encode_structural_field(
    writer: &mut Writer,
    field: &StructuralFieldDeclaration,
) -> Result<(), CodecError> {
    writer.id(field.id);
    writer.string("structural field identity", &field.identity)?;
    writer.u8(match field.relevance {
        BindingRelevance::Relevant => 1,
        BindingRelevance::Erased => 2,
    });
    match &field.field_type {
        StructuralFieldType::Scalar(scalar_type) => {
            writer.u8(1);
            encode_scalar_type(writer, *scalar_type);
        }
        StructuralFieldType::IeeeFloat(format) => {
            writer.u8(4);
            encode_ieee_float_format(writer, *format);
        }
        StructuralFieldType::ByteSequence(carrier) => {
            writer.u8(5);
            encode_byte_sequence_carrier(writer, *carrier);
        }
        StructuralFieldType::Structural(structural_type) => {
            writer.u8(2);
            writer.id(*structural_type);
        }
        StructuralFieldType::Erased { type_identity } => {
            writer.u8(3);
            writer.string("erased structural field type identity", type_identity)?;
        }
    }
    Ok(())
}

pub(super) fn decode_structural_field(
    reader: &mut Reader<'_>,
) -> Result<StructuralFieldDeclaration, CodecError> {
    let id = reader.id("StructuralFieldId")?;
    let identity = reader.string("structural field identity")?;
    let relevance = match reader.u8()? {
        1 => BindingRelevance::Relevant,
        2 => BindingRelevance::Erased,
        tag => return Err(CodecError::InvalidTag("BindingRelevance", tag)),
    };
    let field_type = match reader.u8()? {
        1 => StructuralFieldType::Scalar(decode_scalar_type(reader)?),
        2 => StructuralFieldType::Structural(reader.id("StructuralTypeId")?),
        3 => StructuralFieldType::Erased {
            type_identity: reader.string("erased structural field type identity")?,
        },
        4 => StructuralFieldType::IeeeFloat(decode_ieee_float_format(reader)?),
        5 => StructuralFieldType::ByteSequence(decode_byte_sequence_carrier(reader)?),
        tag => return Err(CodecError::InvalidTag("StructuralFieldType", tag)),
    };
    Ok(StructuralFieldDeclaration {
        id,
        identity,
        relevance,
        field_type,
    })
}
