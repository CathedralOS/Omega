//! Wire encoding of structural places: arguments, paths, place kinds,
//! affine cleanup actions, and obligation identity rosters shared by the
//! module, contract, and dynamic-dispatch sections.

use crate::codec_error::CodecError;
use crate::sections::semantic_module::scalar_wire::{decode_scalar_type, encode_scalar_type};
use crate::sections::semantic_module::structural_signature_wire;
use crate::sections::semantic_module::wire::{
    Reader, Writer, decode_counted, decode_ids, decode_optional_id, encode_optional_id,
};

use semantic_vocabulary::{ObligationId, StructuralPlaceKind};
use terminal_psi::{
    NominalAffineCleanup, StructuralAffineDiscard, StructuralArgument, StructuralPathSegment,
    TerminalAffineCleanupAction, TerminalIdentityIndex,
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
        TerminalAffineCleanupAction::RetireHome(place) => {
            writer.u8(4);
            writer.id(*place);
        }
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            writer.u8(2);
            writer.id(discard.place);
            encode_structural_path(writer, "affine cleanup residual path", &discard.path)?;
            writer.id(discard.structural_type);
        }
        TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
            writer.u8(if cleanup.retains_descendants { 5 } else { 3 });
            writer.id(cleanup.place);
            encode_structural_path(writer, "nominal cleanup path", &cleanup.path)?;
            writer.id(cleanup.structural_type);
            writer.id(cleanup.cleanup_machine);
            writer.len(
                "nominal cleanup consumed claims",
                cleanup.consumed_claims.len(),
            )?;
            for claim in &cleanup.consumed_claims {
                writer.id(*claim);
            }
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
    require_operand_path(path)?;
    writer.len(label, path.len())?;
    for segment in path {
        encode_projection(writer, segment);
    }
    Ok(())
}

/// Result-source metadata alone may carry a declaration interpretation.
/// Every other wire owner uses the ordinary entrance, including block bindings.
pub(crate) fn encode_reference_origin_path(
    writer: &mut Writer,
    label: &'static str,
    path: &[terminal_psi::ReferenceOriginProjection],
) -> Result<(), CodecError> {
    writer.len(label, path.len())?;
    for projection in path {
        match projection {
            terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                if matches!(segment, StructuralPathSegment::AllElements) {
                    return crate::codec_error::malformed(
                        "qualification wildcard is not a reference origin",
                    );
                }
                encode_projection(writer, segment)
            }
            terminal_psi::ReferenceOriginProjection::Element => writer.u8(9),
        }
    }
    Ok(())
}

fn encode_projection(writer: &mut Writer, segment: &StructuralPathSegment) {
    match segment {
        StructuralPathSegment::AllElements => writer.u8(10),
        StructuralPathSegment::Case(case) => {
            writer.u8(7);
            writer.id(*case);
        }
        StructuralPathSegment::Field(identity) => {
            writer.u8(1);
            writer.u32(identity.0);
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
        // Tag 5 carried the retired parameter-selector spelling with a
        // restated interval; it is refused on decode rather than read as
        // this value-and-obligation form.
        StructuralPathSegment::RuntimeIndex { index, obligation } => {
            writer.u8(6);
            writer.id(*index);
            writer.id(*obligation);
        }
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => {
            writer.u8(8);
            writer.id(*structural_type);
            writer.boolean(stored_scalar.is_some());
            if let Some(scalar) = stored_scalar {
                encode_scalar_type(writer, *scalar);
            }
        }
    }
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

pub(crate) fn decode_affine_cleanup_action(
    reader: &mut Reader<'_>,
) -> Result<TerminalAffineCleanupAction, CodecError> {
    match reader.u8()? {
        1 => Ok(TerminalAffineCleanupAction::DiscardRoot(
            reader.id("PlaceId")?,
        )),
        4 => Ok(TerminalAffineCleanupAction::RetireHome(
            reader.id("PlaceId")?,
        )),
        2 => Ok(TerminalAffineCleanupAction::DiscardResidual(
            StructuralAffineDiscard {
                place: reader.id("PlaceId")?,
                path: decode_structural_path(reader)?,
                structural_type: reader.id("StructuralTypeId")?,
            },
        )),
        tag @ (3 | 5) => Ok(TerminalAffineCleanupAction::InvokeNominal(
            NominalAffineCleanup {
                retains_descendants: tag == 5,
                place: reader.id("PlaceId")?,
                path: decode_structural_path(reader)?,
                structural_type: reader.id("StructuralTypeId")?,
                cleanup_machine: reader.id("MachineId")?,
                consumed_claims: decode_ids(reader, "ClaimId")?,
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
    let path = decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        decode_projection(reader, tag)
    })?;
    require_operand_path(&path)?;
    Ok(path)
}

pub(crate) fn decode_reference_origin_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<terminal_psi::ReferenceOriginProjection>, CodecError> {
    decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        if tag == 9 {
            Ok(terminal_psi::ReferenceOriginProjection::Element)
        } else if tag == 10 {
            crate::codec_error::malformed("qualification wildcard is not a reference origin")
        } else {
            decode_projection(reader, tag).map(terminal_psi::ReferenceOriginProjection::Exact)
        }
    })
}

fn decode_projection(
    reader: &mut Reader<'_>,
    tag: u8,
) -> Result<StructuralPathSegment, CodecError> {
    match tag {
        10 => Ok(StructuralPathSegment::AllElements),
        1 => Ok(StructuralPathSegment::Field(TerminalIdentityIndex(
            reader.u32()?,
        ))),
        2 => Ok(StructuralPathSegment::FixedIndex(reader.u64()?)),
        3 => Ok(StructuralPathSegment::Referent),
        4 => Ok(StructuralPathSegment::FixedByteRange {
            start: reader.u64()?,
            end: reader.u64()?,
        }),
        6 => Ok(StructuralPathSegment::RuntimeIndex {
            index: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
        }),
        7 => Ok(StructuralPathSegment::Case(reader.id("StructuralCaseId")?)),
        8 => Ok(StructuralPathSegment::RepresentationView {
            structural_type: reader.id("StructuralTypeId")?,
            stored_scalar: if reader.boolean()? {
                Some(decode_scalar_type(reader)?)
            } else {
                None
            },
        }),
        tag => Err(CodecError::InvalidTag("StructuralPathSegment", tag)),
    }
}

fn require_operand_path(path: &[StructuralPathSegment]) -> Result<(), CodecError> {
    if path.iter().any(|segment| {
        matches!(
            segment,
            StructuralPathSegment::RepresentationView { .. } | StructuralPathSegment::AllElements
        )
    }) {
        return crate::codec_error::malformed(
            "returned-origin interpretation is not an operand or owned path",
        );
    }
    Ok(())
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
mod tests {
    use super::*;
    use semantic_vocabulary::{ClaimId, MachineId, PlaceId, StructuralTypeId};

    #[test]
    fn origin_family_wire_is_distinct_from_an_executable_path() {
        use terminal_psi::ReferenceOriginProjection::{Element, Exact};
        let path = vec![
            Exact(StructuralPathSegment::Field(TerminalIdentityIndex(14))),
            Element,
            Exact(StructuralPathSegment::RepresentationView {
                structural_type: StructuralTypeId::new(2).unwrap(),
                stored_scalar: None,
            }),
            Element,
            Exact(StructuralPathSegment::Referent),
        ];
        let mut writer = Writer::default();
        encode_reference_origin_path(&mut writer, "origin", &path).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_reference_origin_path(&mut reader).unwrap(), path);
        assert_eq!(reader.remaining(), 0);
        assert!(matches!(
            decode_structural_path(&mut Reader::new(&bytes)),
            Err(CodecError::InvalidTag("StructuralPathSegment", 9))
        ));
    }

    #[test]
    fn exact_origin_projection_retains_existing_wire_bytes() {
        let path = vec![
            StructuralPathSegment::Field(TerminalIdentityIndex(14)),
            StructuralPathSegment::FixedIndex(7),
            StructuralPathSegment::Referent,
        ];
        let mut ordinary = Writer::default();
        encode_structural_path(&mut ordinary, "path", &path).unwrap();
        let mut origin = Writer::default();
        let path = path
            .into_iter()
            .map(terminal_psi::ReferenceOriginProjection::Exact)
            .collect::<Vec<_>>();
        encode_reference_origin_path(&mut origin, "origin", &path).unwrap();
        assert_eq!(ordinary.finish(), origin.finish());
    }

    #[test]
    fn nominal_cleanup_round_trip_retains_path_and_descendant_custody() {
        let mut previous_encoding = None;
        for (path, retains_descendants) in [
            (Vec::new(), false),
            (Vec::new(), true),
            (
                vec![StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(14),
                )],
                false,
            ),
            (
                vec![
                    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(14)),
                    StructuralPathSegment::FixedIndex(1),
                ],
                false,
            ),
        ] {
            let action = TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                retains_descendants,
                place: PlaceId::new(1).unwrap(),
                path,
                structural_type: StructuralTypeId::new(2).unwrap(),
                cleanup_machine: MachineId::new(3).unwrap(),
                consumed_claims: vec![ClaimId::new(4).unwrap()],
                cleanup_receiver: Some(PlaceId::new(5).unwrap()),
                requirement_obligations: vec![ObligationId::new(6).unwrap()],
            });
            let mut writer = Writer::default();
            encode_affine_cleanup_action(&mut writer, &action).unwrap();
            let encoded = writer.finish();
            let mut reader = Reader::new(&encoded);
            assert_eq!(decode_affine_cleanup_action(&mut reader).unwrap(), action);
            assert_eq!(reader.remaining(), 0);
            if let Some(previous) = previous_encoding {
                assert_ne!(encoded, previous);
            }
            previous_encoding = Some(encoded);
        }
    }

    #[test]
    fn nominal_cleanup_path_wire_change_rejects_previous_module_version() {
        let mut encoded = super::super::MAGIC.to_vec();
        crate::codec_bytes::push_encoded(&mut encoded, 120_u16.to_le_bytes());
        assert!(matches!(
            super::super::decode_module(&encoded),
            Err(CodecError::UnsupportedFormatMarker(120))
        ));
    }
}

/// Qualification patterns have a separate wire entrance. The ordinary operand
/// entrance rejects universal metadata even when its type could be resolved.
pub(crate) fn encode_qualification_path(
    writer: &mut Writer,
    path: &[StructuralPathSegment],
) -> Result<(), CodecError> {
    writer.len("qualification pattern", path.len())?;
    for step in path {
        encode_projection(writer, step);
    }
    Ok(())
}
pub(crate) fn decode_qualification_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        decode_projection(reader, tag)
    })
}
