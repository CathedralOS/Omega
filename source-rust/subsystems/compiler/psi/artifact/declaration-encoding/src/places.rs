use crate::{
    CodecError,
    terms::scalar_types::{decode_scalar_type, encode_scalar_type},
    wire::{Reader, Writer, decode_counted},
};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};
pub fn encode_structural_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    require_operand_path(path)?;
    encode_projection_path(writer, label, path)
}

fn encode_projection_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    writer.len(label, path.len())?;
    for segment in path {
        encode_projection(writer, segment)?;
    }
    Ok(())
}

pub fn encode_reference_origin_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[terminal_psi::ReferenceOriginProjection],
) -> Result<(), CodecError> {
    writer.len(label, path.len())?;
    for projection in path {
        match projection {
            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                if matches!(segment, StructuralPathSegment::AllElements) {
                    return crate::malformed("qualification wildcard is not a reference origin");
                }
                encode_projection(writer, segment)?
            }
            terminal_psi::ReferenceOriginProjection::Element => writer.u8(9),
        }
    }
    Ok(())
}

fn encode_projection(
    writer: &mut Writer,
    segment: &StructuralPathSegment,
) -> Result<(), CodecError> {
    terminal_psi::structural_path_encoding::try_encode_structural_path_segment(
        writer,
        segment,
        Writer::bytes,
        encode_scalar_type,
    )
}

pub fn encode_structural_place_kind(writer: &mut Writer, kind: StructuralPlaceKind) {
    match kind {
        StructuralPlaceKind::BlockParameter { block, position } => {
            writer.u8(8);
            writer.id(block);
            writer.u32(position);
        }
        StructuralPlaceKind::Parameter { position } => {
            writer.u8(1);
            writer.u32(position);
        }
        StructuralPlaceKind::Result => writer.u8(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            writer.u8(6);
            writer.id(producer);
            writer.id(structural_type);
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            writer.u8(4);
            writer.u32(declaration_ordinal);
            writer.id(structural_type);
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            writer.u8(5);
            writer.id(attachment);
            writer.id(field);
            writer.id(boundary);
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            writer.u8(if construction.is_some() { 7 } else { 3 });
            writer.u32(declaration_ordinal);
            writer.id(structural_type);
            if let Some(construction) = construction {
                writer.id(construction.root_structural_type);
                writer.u64(construction.index);
            }
        }
    }
}

pub fn decode_structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    let path = decode_projection_path(reader)?;
    require_operand_path(&path)?;
    Ok(path)
}

pub fn decode_reference_origin_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<terminal_psi::ReferenceOriginProjection>, CodecError> {
    decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        if tag == 9 {
            Ok(terminal_psi::ReferenceOriginProjection::Element)
        } else if tag == 10 {
            crate::malformed("qualification wildcard is not a reference origin")
        } else {
            decode_projection(reader, tag).map(terminal_psi::ReferenceOriginProjection::Exact)
        }
    })
}

fn decode_projection(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<StructuralPathSegment, CodecError> {
    use terminal_psi::structural_path_encoding::SegmentTag as Tag;
    match Tag::from_byte(tag).ok_or(CodecError::InvalidTag("StructuralPathSegment", tag))? {
        Tag::AllElements => Ok(StructuralPathSegment::AllElements),
        Tag::Field => Ok(StructuralPathSegment::Field(TerminalIdentityIndex(
            reader.u32()?,
        ))),
        Tag::FixedIndex => Ok(StructuralPathSegment::FixedIndex(reader.u64()?)),
        Tag::Referent => Ok(StructuralPathSegment::Referent),
        Tag::FixedRange => Ok(StructuralPathSegment::FixedRange {
            start: reader.u64()?,
            end: reader.u64()?,
        }),
        Tag::RuntimeIndex => Ok(StructuralPathSegment::RuntimeIndex {
            index: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
        }),
        Tag::Case => Ok(StructuralPathSegment::Case(reader.id("StructuralCaseId")?)),
        Tag::RepresentationView => Ok(StructuralPathSegment::RepresentationView {
            structural_type: reader.id("StructuralTypeId")?,
            stored_scalar: if reader.boolean()? {
                Some(decode_scalar_type(reader)?)
            } else {
                None
            },
        }),
    }
}

fn require_operand_path(path: &[StructuralPathSegment]) -> Result<(), CodecError> {
    if path.iter().any(|segment| {
        matches!(
            segment,
            StructuralPathSegment::RepresentationView { .. } | StructuralPathSegment::AllElements
        )
    }) {
        return crate::malformed("returned-origin interpretation is not an operand or owned path");
    }
    Ok(())
}

pub fn decode_structural_place_kind(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceKind, CodecError> {
    Ok(match reader.u8()? {
        8 => StructuralPlaceKind::BlockParameter {
            block: reader.id("BlockId")?,
            position: reader.u32()?,
        },
        1 => StructuralPlaceKind::Parameter {
            position: reader.u32()?,
        },
        2 => StructuralPlaceKind::Result,
        6 => StructuralPlaceKind::OperationResult {
            producer: reader.id("OperationId")?,
            structural_type: reader.id("StructuralTypeId")?,
        },
        4 => StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
        },
        5 => StructuralPlaceKind::ProviderAttachment {
            attachment: reader.id("StructuralTypeId")?,
            field: reader.id("StructuralFieldId")?,
            boundary: reader.id("BoundaryMachineId")?,
        },
        3 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
            construction: None,
        },
        7 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: reader.u32()?,
            structural_type: reader.id("StructuralTypeId")?,
            construction: Some(semantic_vocabulary::AffineConstructionElement {
                root_structural_type: reader.id("StructuralTypeId")?,
                index: reader.u64()?,
            }),
        },
        tag => return Err(CodecError::InvalidTag("StructuralPlaceKind", tag)),
    })
}

fn decode_projection_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        decode_projection(reader, tag)
    })
}
pub fn encode_qualification_path(
    writer: &mut Writer,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    encode_projection_path(writer, "qualification pattern", path)
}

pub fn decode_qualification_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    decode_projection_path(reader)
}

pub fn encode_declared_lifetime_path(
    writer: &mut Writer,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    require_declared_lifetime_path(path)?;
    encode_projection_path(writer, "declared lifetime path", path)
}

pub fn decode_declared_lifetime_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    let path = decode_projection_path(reader)?;
    require_declared_lifetime_path(&path)?;
    Ok(path)
}

fn require_declared_lifetime_path(path: &[StructuralPathSegment]) -> Result<(), CodecError> {
    if !terminal_psi::declared_lifetime_path_is_static(path) {
        return crate::malformed("declared lifetime path is not a static family");
    }
    Ok(())
}
