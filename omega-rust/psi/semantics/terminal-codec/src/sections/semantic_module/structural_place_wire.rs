//! Wire encoding of structural places: arguments, paths, place kinds,
//! affine cleanup actions, and obligation identity rosters shared by the
//! module, contract, and dynamic-dispatch sections.

use crate::codec_error::CodecError;
use crate::sections::semantic_module::scalar_wire::{decode_integer_value, encode_integer_value};
use crate::sections::semantic_module::structural_signature_wire;
use crate::sections::semantic_module::wire::{
    Reader, Writer, decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};

use semantic_vocabulary::{ObligationId, StructuralPlaceKind};
use terminal_psi::{
    NominalAffineCleanup, StructuralAffineDiscard, StructuralArgument, StructuralPathSegment,
    TerminalAffineCleanupAction,
};

pub(crate) fn encode_structural_arguments(
    writer: &mut Writer,
    arguments: &[StructuralArgument],
) -> Result<(), CodecError> {
    writer.len("structural arguments", arguments.len())?;
    for argument in arguments {
        writer.id(argument.place);
        structural_signature_wire::encode_structural_access(writer, argument.access);
        encode_structural_path(writer, "structural argument path", &argument.path)?;
    }
    Ok(())
}

pub(crate) fn encode_affine_cleanup_action(
    writer: &mut Writer,
    action: &TerminalAffineCleanupAction,
) -> Result<(), CodecError> {
    match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => {
            writer.u8(1);
            writer.id(*place);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            writer.u8(2);
            writer.id(discard.place);
            encode_structural_path(writer, "affine cleanup residual path", &discard.path)?;
            writer.id(discard.structural_type);
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            writer.u8(3);
            writer.id(cleanup.place);
            writer.id(cleanup.structural_type);
            writer.id(cleanup.cleanup_machine);
            encode_optional_id(writer, cleanup.cleanup_receiver);
            encode_obligation_ids(writer, &cleanup.requirement_obligations)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_structural_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    writer.len(label, path.len())?;
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                writer.u8(1);
                writer.string("structural path field", identity)?;
            }
            StructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
            StructuralPathSegment::Referent => writer.u8(3),
            StructuralPathSegment::FixedByteRange { start, end } => {
                writer.u8(4);
                writer.u64(*start);
                writer.u64(*end);
            }
            StructuralPathSegment::RuntimeIndex {
                selector,
                minimum,
                maximum,
            } => {
                writer.u8(5);
                writer.u32(*selector);
                encode_integer_value(writer, *minimum);
                encode_integer_value(writer, *maximum);
            }
        }
    }
    Ok(())
}

pub(crate) fn encode_obligation_ids(
    writer: &mut Writer,
    obligations: &[ObligationId],
) -> Result<(), CodecError> {
    writer.len("requirement obligations", obligations.len())?;
    for obligation in obligations {
        writer.id(*obligation);
    }
    Ok(())
}

pub(crate) fn encode_structural_place_kind(writer: &mut Writer, kind: StructuralPlaceKind) {
    match kind {
        StructuralPlaceKind::BlockParameter { block, position } => {
            writer.u8(8);
            writer.id(block);
            writer.u32(position);
        }
        StructuralPlaceKind::Parameter { position, is_self } => {
            writer.u8(1);
            writer.u32(position);
            writer.u8(u8::from(is_self));
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

pub(crate) fn decode_affine_cleanup_action(
    reader: &mut Reader<'_>,
) -> Result<TerminalAffineCleanupAction, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalAffineCleanupAction::DiscardRoot(
            reader.id("PlaceId")?,
        )),
        2 => Ok(TerminalAffineCleanupAction::DiscardResidual(
            StructuralAffineDiscard {
                place: reader.id("PlaceId")?,
                path: decode_structural_path(reader)?,
                structural_type: reader.id("StructuralTypeId")?,
            },
        )),
        3 => Ok(TerminalAffineCleanupAction::InvokeNominal(
            NominalAffineCleanup {
                place: reader.id("PlaceId")?,
                structural_type: reader.id("StructuralTypeId")?,
                cleanup_machine: reader.id("MachineId")?,
                cleanup_receiver: decode_optional_id(reader, "PlaceId")?,
                requirement_obligations: decode_ids(reader, "ObligationId")?,
            },
        )),
        tag => Err(CodecError::InvalidTag("TerminalAffineCleanupAction", tag)),
    }
}

pub(crate) fn decode_structural_arguments(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralArgument>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(StructuralArgument {
            place: reader.id("PlaceId")?,
            access: structural_signature_wire::decode_structural_access(reader)?,
            path: decode_structural_path(reader)?,
        })
    })
}

pub(crate) fn decode_structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(StructuralPathSegment::Field(
            reader.string("structural path field")?,
        )),
        2 => Ok(StructuralPathSegment::FixedIndex(reader.u64()?)),
        3 => Ok(StructuralPathSegment::Referent),
        4 => Ok(StructuralPathSegment::FixedByteRange {
            start: reader.u64()?,
            end: reader.u64()?,
        }),
        5 => Ok(StructuralPathSegment::RuntimeIndex {
            selector: reader.u32()?,
            minimum: decode_integer_value(reader)?,
            maximum: decode_integer_value(reader)?,
        }),
        tag => Err(CodecError::InvalidTag("StructuralPathSegment", tag)),
    })
}

pub(crate) fn decode_structural_place_kind(
    reader: &mut Reader<'_>,
) -> Result<StructuralPlaceKind, CodecError> {
    Ok(match reader.u8()? {
        8 => StructuralPlaceKind::BlockParameter {
            block: reader.id("BlockId")?,
            position: reader.u32()?,
        },
        1 => StructuralPlaceKind::Parameter {
            position: reader.u32()?,
            is_self: reader.boolean()?,
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

#[cfg(test)]
mod structural_place_wire_tests {
    use semantic_vocabulary::{OperationId, PsiSemanticId, StructuralPlaceKind, StructuralTypeId};

    use super::{CodecError, decode_structural_place_kind, encode_structural_place_kind};
    use crate::sections::semantic_module::wire::{Reader, Writer};

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test ids are nonzero")
    }

    #[test]
    fn block_parameter_place_encodes_exact_block_and_dense_position() {
        let kind = StructuralPlaceKind::BlockParameter {
            block: id(17),
            position: 0,
        };
        let mut writer = Writer::default();
        encode_structural_place_kind(&mut writer, kind);
        let bytes = writer.finish();
        assert_eq!(
            bytes,
            [&[8][..], &17_u64.to_le_bytes(), &0_u32.to_le_bytes(),].concat()
        );
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_place_kind(&mut reader), Ok(kind));
        assert_eq!(reader.remaining(), 0);
        let mut zero_block = bytes.clone();
        zero_block[1..9].fill(0);
        assert!(decode_structural_place_kind(&mut Reader::new(&zero_block)).is_err());
        for length in 0..bytes.len() {
            assert!(decode_structural_place_kind(&mut Reader::new(&bytes[..length])).is_err());
        }
    }

    #[test]
    fn operation_result_place_uses_stable_wire_tag_six() {
        let kind = StructuralPlaceKind::OperationResult {
            producer: id::<OperationId>(1),
            structural_type: id::<StructuralTypeId>(2),
        };
        let mut writer = Writer::default();
        encode_structural_place_kind(&mut writer, kind);
        let bytes = writer.finish();
        assert_eq!(bytes[0], 6);

        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_place_kind(&mut reader), Ok(kind));
        assert_eq!(reader.remaining(), 0);

        let mut invalid = bytes;
        invalid[0] = 9;
        assert_eq!(
            decode_structural_place_kind(&mut Reader::new(&invalid)),
            Err(CodecError::InvalidTag("StructuralPlaceKind", 9))
        );
    }
}
