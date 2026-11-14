/*
Canonical wire encoders and decoders for structural type declarations, fields, places, paths,
operation results, and machine structural signatures.
*/

pub(crate) use crate::semantic_module::*;

pub(crate) mod structural_field_wire {

    use semantic_vocabulary::{
        BoundedIntegerType, ByteSequenceStructuralField, CanonicalStructuralPathSegment,
        IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, PlaceId,
        ScalarRefinement,
    };
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
    };

    use super::CodecError;
    use super::proposition_wire::{decode_proposition, encode_proposition};
    use super::scalar_wire::{
        decode_integer_type, decode_integer_value, decode_scalar_type, encode_integer_type,
        encode_integer_value, encode_scalar_type,
    };
    use super::wire::{Reader, Writer, decode_counted};
    use terminal_psi::TerminalIdentityIndex;

    pub(crate) fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
        writer.u8(match format {
            IeeeFloatFormat::Binary32 => 1,
            IeeeFloatFormat::Binary64 => 2,
        });
    }

    pub(crate) fn encode_ieee_float_comparison_kind(
        writer: &mut Writer,
        kind: IeeeFloatComparisonKind,
    ) {
        writer.u8(match kind {
            IeeeFloatComparisonKind::Equal => 1,
            IeeeFloatComparisonKind::NotEqual => 2,
            IeeeFloatComparisonKind::LessThan => 3,
            IeeeFloatComparisonKind::LessOrEqual => 4,
            IeeeFloatComparisonKind::NotLessThan => 5,
            IeeeFloatComparisonKind::NotLessOrEqual => 6,
        });
    }

    pub(crate) fn decode_ieee_float_comparison_kind(
        reader: &mut Reader<'_>,
    ) -> Result<IeeeFloatComparisonKind, CodecError> {
        match reader.u8()? {
            1 => Ok(IeeeFloatComparisonKind::Equal),
            2 => Ok(IeeeFloatComparisonKind::NotEqual),
            3 => Ok(IeeeFloatComparisonKind::LessThan),
            4 => Ok(IeeeFloatComparisonKind::LessOrEqual),
            5 => Ok(IeeeFloatComparisonKind::NotLessThan),
            6 => Ok(IeeeFloatComparisonKind::NotLessOrEqual),
            tag => Err(CodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
        }
    }

    pub(crate) fn decode_ieee_float_format(
        reader: &mut Reader<'_>,
    ) -> Result<IeeeFloatFormat, CodecError> {
        match reader.u8()? {
            1 => Ok(IeeeFloatFormat::Binary32),
            2 => Ok(IeeeFloatFormat::Binary64),
            tag => Err(CodecError::InvalidTag("IeeeFloatFormat", tag)),
        }
    }

    pub(crate) fn encode_ieee_float_field(
        writer: &mut Writer,
        field: &IeeeFloatStructuralField,
    ) -> Result<(), CodecError> {
        encode_canonical_structural_field(
            writer,
            field.root(),
            field.path(),
            "IEEE float field path",
        )
    }

    pub(crate) fn decode_ieee_float_field(
        reader: &mut Reader<'_>,
    ) -> Result<IeeeFloatStructuralField, CodecError> {
        let (root, path) = decode_canonical_structural_field(reader)?;
        IeeeFloatStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
    }

    pub(crate) fn encode_byte_sequence_field(
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

    pub(crate) fn decode_byte_sequence_field(
        reader: &mut Reader<'_>,
    ) -> Result<ByteSequenceStructuralField, CodecError> {
        let (root, path) = decode_canonical_structural_field(reader)?;
        ByteSequenceStructuralField::new(root, path).map_err(CodecError::MalformedProposition)
    }

    pub(crate) fn encode_canonical_structural_field(
        writer: &mut Writer,
        root: PlaceId,
        path: &[CanonicalStructuralPathSegment],
        length_label: &'static str,
    ) -> Result<(), CodecError> {
        writer.id(root);
        encode_canonical_structural_path(writer, path, length_label)
    }

    pub(crate) fn encode_canonical_structural_path(
        writer: &mut Writer,
        path: &[CanonicalStructuralPathSegment],
        length_label: &'static str,
    ) -> Result<(), CodecError> {
        writer.len(length_label, path.len())?;
        for segment in path {
            match segment {
                CanonicalStructuralPathSegment::Referent => writer.u8(4),
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

    pub(crate) fn decode_canonical_structural_field(
        reader: &mut Reader<'_>,
    ) -> Result<(PlaceId, Vec<CanonicalStructuralPathSegment>), CodecError> {
        let root = reader.id("PlaceId")?;
        let path = decode_counted(reader, |reader| match reader.u8()? {
            1 => Ok(CanonicalStructuralPathSegment::Field(
                reader.id("StructuralFieldId")?,
            )),
            2 => Ok(CanonicalStructuralPathSegment::FixedIndex(reader.u64()?)),
            3 => Ok(CanonicalStructuralPathSegment::Case(
                reader.id("StructuralCaseId")?,
            )),
            4 => Ok(CanonicalStructuralPathSegment::Referent),
            tag => Err(CodecError::InvalidTag(
                "CanonicalStructuralPathSegment",
                tag,
            )),
        })?;
        Ok((root, path))
    }

    pub(crate) fn encode_byte_sequence_carrier(writer: &mut Writer, carrier: ByteSequenceCarrier) {
        match carrier {
            ByteSequenceCarrier::BorrowedView { access } => {
                writer.u8(1);
                match access {
                    Some(access) => {
                        super::structural_signature_wire::encode_structural_access(writer, access);
                    }
                    None => writer.u8(0),
                }
            }
            ByteSequenceCarrier::BoundedView { access, capacity } => {
                writer.u8(3);
                match access {
                    Some(access) => {
                        super::structural_signature_wire::encode_structural_access(writer, access);
                    }
                    None => writer.u8(0),
                }
                writer.u64(capacity);
            }
            ByteSequenceCarrier::BoundedOwned { capacity } => {
                writer.u8(2);
                writer.u64(capacity);
            }
        }
    }

    pub(crate) fn decode_byte_sequence_carrier(
        reader: &mut Reader<'_>,
    ) -> Result<ByteSequenceCarrier, CodecError> {
        match reader.u8()? {
            1 => Ok(ByteSequenceCarrier::BorrowedView {
                access: super::structural_signature_wire::decode_optional_structural_access(
                    reader,
                )?,
            }),
            2 => Ok(ByteSequenceCarrier::BoundedOwned {
                capacity: reader.u64()?,
            }),
            3 => Ok(ByteSequenceCarrier::BoundedView {
                access: super::structural_signature_wire::decode_optional_structural_access(
                    reader,
                )?,
                capacity: reader.u64()?,
            }),
            tag => Err(CodecError::InvalidTag("ByteSequenceCarrier", tag)),
        }
    }

    pub(crate) fn encode_structural_field(
        writer: &mut Writer,
        field: &StructuralFieldDeclaration,
    ) -> Result<(), CodecError> {
        writer.id(field.id);
        writer.u32(field.identity.0);
        writer.u8(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        super::wire::encode_optional_id(writer, field.inline_carrier);
        encode_structural_field_type(writer, &field.field_type)
    }

    pub(crate) fn encode_structural_field_type(
        writer: &mut Writer,
        field_type: &StructuralFieldType,
    ) -> Result<(), CodecError> {
        match field_type {
            StructuralFieldType::AtomicBoundedInteger(bounded) => {
                writer.u8(10);
                encode_integer_type(writer, bounded.integer_type())?;
                encode_integer_value(writer, bounded.minimum());
                encode_integer_value(writer, bounded.maximum());
            }
            StructuralFieldType::Atomic(scalar_type) => {
                writer.u8(8);
                encode_scalar_type(writer, *scalar_type)?;
            }
            StructuralFieldType::Scalar(scalar_type) => {
                writer.u8(1);
                encode_scalar_type(writer, *scalar_type)?;
            }
            StructuralFieldType::BoundedInteger(bounded) => {
                writer.u8(6);
                encode_integer_type(writer, bounded.integer_type())?;
                encode_integer_value(writer, bounded.minimum());
                encode_integer_value(writer, bounded.maximum());
            }
            StructuralFieldType::RefinedScalar(refinement) => {
                writer.u8(9);
                encode_scalar_type(writer, refinement.carrier())?;
                encode_proposition(writer, refinement.predicate())?;
            }
            StructuralFieldType::IeeeFloat(format) => {
                writer.u8(4);
                encode_ieee_float_format(writer, *format);
            }
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } => {
                writer.u8(7);
                encode_byte_sequence_carrier(writer, *carrier);
                writer.u8(predicates.bits());
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
                writer.u32(type_identity.0);
            }
        }
        Ok(())
    }

    pub(crate) fn decode_structural_field(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralFieldDeclaration, CodecError> {
        let id = reader.id("StructuralFieldId")?;
        let identity = TerminalIdentityIndex(reader.u32()?);
        let relevance = match reader.u8()? {
            1 => BindingRelevance::Relevant,
            2 => BindingRelevance::Erased,
            tag => return Err(CodecError::InvalidTag("BindingRelevance", tag)),
        };
        let inline_carrier = super::wire::decode_optional_id(reader, "StructuralTypeId")?;
        let field_type = decode_structural_field_type(reader)?;
        Ok(StructuralFieldDeclaration {
            inline_carrier,
            storage: None,
            id,
            identity,
            relevance,
            field_type,
        })
    }

    pub(crate) fn decode_structural_field_type(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralFieldType, CodecError> {
        Ok(match reader.u8()? {
            1 => StructuralFieldType::Scalar(decode_scalar_type(reader)?),
            8 => StructuralFieldType::Atomic(decode_scalar_type(reader)?),
            10 => StructuralFieldType::AtomicBoundedInteger(
                BoundedIntegerType::new(
                    decode_integer_type(reader)?,
                    decode_integer_value(reader)?,
                    decode_integer_value(reader)?,
                )
                .map_err(CodecError::MalformedProposition)?,
            ),
            9 => StructuralFieldType::RefinedScalar(
                ScalarRefinement::new(decode_scalar_type(reader)?, decode_proposition(reader)?)
                    .map_err(|_| {
                        CodecError::MalformedStructuralFoundation(
                            "scalar field predicate is not closed and carrier-typed",
                        )
                    })?,
            ),
            2 => StructuralFieldType::Structural(reader.id("StructuralTypeId")?),
            3 => StructuralFieldType::Erased {
                type_identity: TerminalIdentityIndex(reader.u32()?),
            },
            4 => StructuralFieldType::IeeeFloat(decode_ieee_float_format(reader)?),
            5 => StructuralFieldType::ByteSequence(decode_byte_sequence_carrier(reader)?),
            6 => StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    decode_integer_type(reader)?,
                    decode_integer_value(reader)?,
                    decode_integer_value(reader)?,
                )
                .map_err(CodecError::MalformedProposition)?,
            ),
            7 => {
                let carrier = decode_byte_sequence_carrier(reader)?;
                let bits = reader.u8()?;
                let predicates = terminal_psi::ByteSequencePredicateSet::from_bits(bits)
                    .filter(|predicates| !predicates.is_empty())
                    .ok_or(CodecError::InvalidTag("ByteSequencePredicateSet", bits))?;
                StructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates,
                }
            }
            tag => return Err(CodecError::InvalidTag("StructuralFieldType", tag)),
        })
    }

    #[cfg(test)]
    mod reference_path_wire_tests {
        use super::super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
        use semantic_vocabulary::{
            ScalarReadProjection, ScalarTerm, ScalarType, StructuralFieldId,
        };

        use super::*;

        #[test]
        fn referent_paths_round_trip_in_both_canonical_tag_spaces() {
            let root = PlaceId::new(1).unwrap();
            let field = StructuralFieldId::new(2).unwrap();
            let path = vec![
                CanonicalStructuralPathSegment::Referent,
                CanonicalStructuralPathSegment::Field(field),
            ];
            let mut writer = Writer::default();
            encode_canonical_structural_field(&mut writer, root, &path, "test reference path")
                .unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                decode_canonical_structural_field(&mut reader).unwrap(),
                (root, path.clone())
            );
            assert_eq!(reader.remaining(), 0);
            let term = ScalarTerm::StructuralRead {
                root,
                path: path.into_iter().map(ScalarReadProjection::from).collect(),
                scalar_type: ScalarType::Boolean,
            };
            let mut writer = Writer::default();
            encode_scalar_term(&mut writer, &term).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_scalar_term(&mut reader).unwrap(), term);
            assert_eq!(reader.remaining(), 0);
        }
    }
}

pub(crate) mod structural_place_wire {

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
                writer.ids("nominal cleanup consumed claims", &cleanup.consumed_claims)?;
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
            encode_projection(writer, segment)?;
        }
        Ok(())
    }

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

    pub(crate) fn encode_obligation_ids(
        writer: &mut Writer,
        obligations: &[ObligationId],
    ) -> Result<(), CodecError> {
        writer.ids("requirement obligations", obligations)?;
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
                StructuralPathSegment::RepresentationView { .. }
                    | StructuralPathSegment::AllElements
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

    pub(crate) fn encode_qualification_path(
        writer: &mut Writer,
        path: &[StructuralPathSegment],
    ) -> Result<(), CodecError> {
        writer.len("qualification pattern", path.len())?;
        for step in path {
            encode_projection(writer, step)?;
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
}

pub(crate) mod structural_result_wire {

    use terminal_psi::{
        ResultQualificationEstablishment, StructuralMultiplicity, StructuralOperationResult,
        StructuralResultDeclaration,
    };

    use super::CodecError;
    use super::structural_signature_wire::{
        decode_projected_qualifications, encode_projected_qualifications,
    };
    use super::wire::{Reader, Writer, strictly_increasing};
    use super::wire_fields::{
        ClaimBinding, Field, Id, Ids, List, Multiplicity, PathQualification, wire_record,
    };
    use crate::sections::semantic_module::structural_place_wire::{
        decode_reference_origin_path, decode_structural_path, encode_reference_origin_path,
        encode_structural_path,
    };
    use crate::sections::semantic_module::wire::{decode_counted, decode_ids};

    pub(crate) fn validate_reference_sources(
        module: &terminal_psi::TerminalModule,
        machine: &terminal_psi::TerminalMachine,
    ) -> Result<(), CodecError> {
        let Some(result) = machine.result.structural() else {
            return Ok(());
        };
        validate_reference_source_order(result)?;
        for reference in &result.reference_sources {
            let reference_type =
                crate::sections::semantic_module::module_foundation_validation::validate_structural_path(
                    module,
                    result.structural_type,
                    &reference.path,
                )?;
            let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) =
                module.structural_type(reference_type).map(|row| &row.shape)
            else {
                return crate::codec_error::malformed(
                    "reference result source must identify a reference leaf",
                );
            };
            for source in &reference.sources {
                let root = match source {
                    terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                        machine
                            .signature_structural_parameter(source.place)
                            .ok_or(CodecError::MalformedStructuralFoundation(
                                "reference result source must identify a formal ingress",
                            ))?
                            .structural_type
                    }
                    terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type, ..
                    } => *carrier_type,
                };
                let endpoint =
                    crate::sections::semantic_module::module_foundation_validation::validate_reference_origin_family(
                        module, root, source.path(),
                    )?;
                if *access != source.access()
                    || (source.extent() == semantic_vocabulary::ReferenceOriginExtent::Projected
                        && *referent != endpoint)
                {
                    return crate::codec_error::malformed(
                        "reference result source has mismatched referent or access",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_reference_source_order(
        result: &StructuralResultDeclaration,
    ) -> Result<(), CodecError> {
        if !strictly_increasing(result.reference_sources.iter().map(|source| &source.path)) {
            return crate::codec_error::malformed(
                "reference result sources must have unique ordered paths",
            );
        }
        for reference in &result.reference_sources {
            if reference.sources.is_empty() || !strictly_increasing(reference.sources.iter()) {
                return crate::codec_error::malformed(
                    "reference result alternatives must be nonempty, unique and ordered",
                );
            }
        }
        Ok(())
    }

    pub(crate) fn encode_function_result(
        writer: &mut Writer,
        result: &StructuralResultDeclaration,
    ) -> Result<(), CodecError> {
        writer.id(result.place);
        writer.id(result.structural_type);
        encode_multiplicity(writer, result.multiplicity);
        writer.ids("structural result qualifications", &result.qualifications)?;
        encode_projected_qualifications(writer, &result.projected_qualifications)?;
        writer.len(
            "structural reference result sources",
            result.reference_sources.len(),
        )?;
        for reference in &result.reference_sources {
            encode_structural_path(writer, "reference result path", &reference.path)?;
            writer.len("reference source alternatives", reference.sources.len())?;
            for source in &reference.sources {
                match source {
                    terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                        writer.u8(1);
                        writer.id(source.place);
                    }
                    terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type, ..
                    } => {
                        writer.u8(2);
                        writer.id(*carrier_type);
                    }
                }
                super::structural_signature_wire::encode_structural_access(writer, source.access());
                writer.u8(match source.extent() {
                    semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
                    semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
                });
                encode_reference_origin_path(writer, "reference source path", source.path())?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode_function_result(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralResultDeclaration, CodecError> {
        let result = StructuralResultDeclaration {
            place: reader.id("PlaceId")?,
            structural_type: reader.id("StructuralTypeId")?,
            multiplicity: decode_multiplicity(reader)?,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
            projected_qualifications: decode_projected_qualifications(reader)?,
            reference_sources: decode_counted(reader, |reader| {
                Ok(terminal_psi::StructuralReferenceResultSource {
                    path: decode_structural_path(reader)?,
                    sources: decode_counted(reader, |reader| {
                        let kind = reader.u8()?;
                        let (parameter, carrier) = match kind {
                            1 => (Some(reader.id("PlaceId")?), None),
                            2 => (None, Some(reader.id("StructuralTypeId")?)),
                            tag => {
                                return Err(CodecError::InvalidTag(
                                    "StructuralReferenceOrigin",
                                    tag,
                                ));
                            }
                        };
                        let access =
                            super::structural_signature_wire::decode_structural_access(reader)?;
                        let extent = match reader.u8()? {
                            1 => semantic_vocabulary::ReferenceOriginExtent::Projected,
                            2 => semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                            tag => {
                                return Err(CodecError::InvalidTag("ReferenceOriginExtent", tag));
                            }
                        };
                        let path = decode_reference_origin_path(reader)?;
                        match kind {
                            1 => Ok(terminal_psi::StructuralReferenceOrigin::Parameter(
                                terminal_psi::ReferenceOriginSource {
                                    place: parameter.ok_or(CodecError::NonCanonicalEncoding)?,
                                    access,
                                    extent,
                                    path,
                                },
                            )),
                            2 => Ok(terminal_psi::StructuralReferenceOrigin::Invocation {
                                carrier_type: carrier.ok_or(CodecError::NonCanonicalEncoding)?,
                                access,
                                extent,
                                path,
                            }),
                            tag => Err(CodecError::InvalidTag("StructuralReferenceOrigin", tag)),
                        }
                    })?,
                })
            })?,
        };
        validate_reference_source_order(&result)?;
        Ok(result)
    }

    wire_record! {
        OperationResult: StructuralOperationResult {
            place: Id,
            structural_type: Id,
            multiplicity: Multiplicity,
            qualifications: Ids,
            projected_qualifications: List<PathQualification>,
            additional_input_qualifications: List<PathQualification>,
            qualification_establishments: List<Establishment>,
            claims: List<ClaimBinding>,
        }
        Establishment: ResultQualificationEstablishment { domain: Id, route: RouteIndex }
    }

    struct RouteIndex;
    impl Field<u32> for RouteIndex {
        fn write(writer: &mut Writer, value: &u32) -> Result<(), CodecError> {
            writer.u64(u64::from(*value));
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<u32, CodecError> {
            u32::try_from(reader.u64()?)
                .map_err(|_| CodecError::MalformedStructuralFoundation("route index out of range"))
        }
    }

    pub(crate) fn encode_operation_result(
        writer: &mut Writer,
        result: &StructuralOperationResult,
    ) -> Result<(), CodecError> {
        OperationResult::write(writer, result)
    }

    pub(crate) fn decode_operation_result(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralOperationResult, CodecError> {
        OperationResult::read(reader)
    }

    pub(crate) fn encode_multiplicity(writer: &mut Writer, multiplicity: StructuralMultiplicity) {
        writer.u8(match multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
    }

    pub(crate) fn decode_multiplicity(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralMultiplicity, CodecError> {
        match reader.u8()? {
            1 => Ok(StructuralMultiplicity::Unrestricted),
            2 => Ok(StructuralMultiplicity::Affine),
            3 => Ok(StructuralMultiplicity::Linear),
            tag => Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        }
    }
}

pub(crate) mod structural_signature_wire {

    use semantic_vocabulary::{
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
        ContentProjectionScalar, QualifiedScalarType, ServiceId,
    };
    use terminal_psi::{
        BoundaryCompletion, BoundaryContentGuarantee, BoundaryDeclarationRole,
        BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
        BoundaryReferenceOutput, BoundaryRequirementKind, BoundaryStructuralResultDeclaration,
        CheckedBoundaryCallable, CheckedBoundaryExecution, ExternalCompletionKind,
        ProgramLocalRootIntroductionSchema, RetainedBorrowContentProjection, RetainedBorrowCustody,
        RetainedBorrowPlace, RetainedBorrowPlaceRoot, ScalarDomainRequirement, StructuralAccess,
        StructuralDomainRequirement, StructuralParameterDeclaration, StructuralPathQualification,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        Access, ContentAlgebra, ContentExpression, ContentProjection, CrashRoutes, Field, Flag, Id,
        Ids, Index, List, MachineDeclaration, Multiplicity, Nominal, OptionalId, PathQualification,
        ProjectionIdentity, Proposition, QualificationPath, QualificationSet, ReachArgument,
        Scalar, Text, U32, U64, wire_enum, wire_record,
    };
    use crate::sections::semantic_module::wire::decode_counted;
    use crate::term_table::{TableChildren, TableTerm, decode_term_table, encode_term_table};

    pub(crate) mod reference_outputs {

        use super::super::structural_place_wire::{decode_structural_path, encode_structural_path};
        use super::{
            CodecError, Reader, Writer, decode_counted, decode_structural_access,
            encode_structural_access,
        };
        use terminal_psi::{
            BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
        };

        pub(crate) fn encode(
            writer: &mut Writer,
            outputs: &[BoundaryReferenceOutput],
        ) -> Result<(), CodecError> {
            writer.len("boundary reference outputs", outputs.len())?;
            for output in outputs {
                match output.destination {
                    BoundaryReferenceDestination::Parameter(position) => {
                        writer.u8(0);
                        writer.u32(position);
                    }
                }
                encode_structural_path(writer, "boundary reference output path", &output.path)?;
                encode_structural_access(writer, output.access);
                writer.len(
                    "boundary reference lifetime dependencies",
                    output.dependencies.len(),
                )?;
                for dependency in &output.dependencies {
                    match dependency {
                        BoundaryLifetimeDependency::Referent {
                            argument_index,
                            access,
                        } => {
                            writer.u8(0);
                            writer.u32(*argument_index);
                            encode_structural_access(writer, *access);
                        }
                        BoundaryLifetimeDependency::Stored {
                            argument_index,
                            path,
                            access,
                        } => {
                            writer.u8(1);
                            writer.u32(*argument_index);
                            encode_structural_access(writer, *access);
                            encode_structural_path(
                                writer,
                                "boundary reference lifetime path",
                                path,
                            )?;
                        }
                    }
                }
            }
            Ok(())
        }

        pub(crate) fn decode(
            reader: &mut Reader<'_>,
        ) -> Result<Vec<BoundaryReferenceOutput>, CodecError> {
            decode_counted(reader, |reader| {
                let destination = match reader.u8()? {
                    0 => BoundaryReferenceDestination::Parameter(reader.u32()?),
                    tag => return Err(CodecError::InvalidTag("BoundaryReferenceDestination", tag)),
                };
                let path = decode_structural_path(reader)?;
                let access = decode_structural_access(reader)?;
                let dependencies = decode_counted(reader, |reader| match reader.u8()? {
                    0 => Ok(BoundaryLifetimeDependency::Referent {
                        argument_index: reader.u32()?,
                        access: decode_structural_access(reader)?,
                    }),
                    1 => Ok(BoundaryLifetimeDependency::Stored {
                        argument_index: reader.u32()?,
                        access: decode_structural_access(reader)?,
                        path: decode_structural_path(reader)?,
                    }),
                    tag => Err(CodecError::InvalidTag("BoundaryLifetimeDependency", tag)),
                })?;
                Ok(BoundaryReferenceOutput {
                    destination,
                    path,
                    access,
                    dependencies,
                })
            })
        }
    }

    wire_record! {
        BoundaryMachine: BoundaryMachineDeclaration {
            id: Id,
            declaration: OptionalCallable,
            numeric_intrinsic: NumericIntrinsic,
            identity: Index,
            owner: Nominal,
            role: Role,
            kind: RequirementKind,
            completion: Completion,
            unconditional_termination: Flag,
            attachment: OptionalId,
            parameter_order: List<ParameterKind>,
            scalar_parameters: List<Scalar>,
            crash_routes: CrashRoutes,
            structural_parameters: List<StructuralParameter>,
            result: CallResult,
            result_reference_regions: ReferenceRegions,
            requires: List<DomainRequirement>,
            normal_qualifications: List<DomainRequirement>,
            scalar_domain_requirements: List<ScalarDomain>,
            scalar_requires: List<Proposition>,
            scalar_ensures: List<Proposition>,
            reference_outputs: ReferenceOutputs,
            program_local_root_introductions: List<RootIntroduction>,
            content_guarantees: List<ContentGuarantee>,
            fixed_service_reach: Ids,
            owning_service: OptionalId,
            published_service_ceiling: Ids,
            published_synchronous_entries: Ids,
        }
        Callable: CheckedBoundaryCallable {
            source: MachineDeclaration,
            method_arguments: List<ReachArgument>,
            execution: Execution,
        }
        DomainRequirement: StructuralDomainRequirement {
            argument_index: U32,
            path: QualificationPath,
            domain: Id,
        }
        ScalarDomain: ScalarDomainRequirement { argument_index: U32, domain: Id }
        RootIntroduction: ProgramLocalRootIntroductionSchema {
            argument_index: U32,
            source_parameter_position: U32,
            qualification: Id,
            carrier: Id,
            projection: ProjectionIdentity,
            algebra: ContentAlgebra,
            capacity: ContentExpression,
            compatibility_report_identity: U64,
        }
        StructuralParameter: StructuralParameterDeclaration {
            place: Id,
            position: U32,
            structural_type: Id,
            multiplicity: Multiplicity,
            access: Access,
            qualifications: Ids,
            projected_qualifications: List<PathQualification>,
            additional_input_qualifications: List<PathQualification>,
        }
        BorrowPlace: RetainedBorrowPlace {
            version: PlaceVersion,
            root: PlaceRoot,
            segments: List<PlaceSegment>,
        }
        BorrowProjection: RetainedBorrowContentProjection {
            semantic_domain: Id,
            carrier_identity: Index,
            projection: ContentProjection,
        }
        BorrowCustody: RetainedBorrowCustody {
            callable_identity: Index,
            source: BorrowPlace,
            result: BorrowPlace,
            access: Access,
            callable_lifetime_parameter_count: U32,
            callable_lifetime_parameter_ordinal: U32,
            result_nominal_identity: Index,
            result_multiplicity: Multiplicity,
            result_lifetime_argument_count: U32,
            result_lifetime_argument_ordinal: U32,
            result_lifetime_slot_is_erased: Flag,
            retained_semantic_domain: Id,
            source_projection: BorrowProjection,
            result_projection: BorrowProjection,
        }
        StructuralResult: BoundaryStructuralResultDeclaration {
            structural_type: Id,
            multiplicity: Multiplicity,
            qualifications: Ids,
            projected_qualifications: List<PathQualification>,
        }
        QualifiedScalar: QualifiedScalarType { scalar_type: Scalar, qualifications: QualificationSet }
    }

    wire_enum! {
        Execution: CheckedBoundaryExecution, "checked boundary execution" {
            0 => Provider,
            1 => AccessInitialize,
            2 => AccessMoveOut,
            3 => AccessView,
            4 => EmptyRunRelease,
            5 => AccessRelocate,
            6 => StaticInterruptInstall,
            7 => StaticInterruptContext,
            8 => StaticInterruptBeginCompletion,
            9 => StaticInterruptFinishCompletion,
        }
        Role: BoundaryDeclarationRole, "BoundaryDeclarationRole" { 0 => Callable, 1 => ContractEvidence }
        RequirementKind: BoundaryRequirementKind, "BoundaryRequirementKind" {
            0 => Machine,
            1 => Trait,
            2 => Operator,
        }
        ParameterKind: BoundaryParameterKind, "BoundaryParameterKind" { 0 => Scalar, 1 => Structural }
        ContentGuarantee: BoundaryContentGuarantee, "BoundaryContentGuarantee" {
            1 => Conservation(guarantee: super::content_wire::Guarantee),
            2 => RetainedBorrow(custody: BorrowCustody),
        }
        PlaceVersion: ContentPlaceVersion, "ContentPlaceVersion" { 1 => Entry, 2 => Current }
        PlaceRoot: RetainedBorrowPlaceRoot, "RetainedBorrowPlaceRoot" {
            1 => Parameter { position: U32, identity: Index },
            2 => Result,
        }
        PlaceSegment: ContentPlaceSegment, "ContentPlaceSegment" {
            1 => Case(identity: Text),
            2 => Field(identity: Text),
            3 => FixedIndex(index: U64),
        }
        CallResult: BoundaryMachineResult, "BoundaryMachineResult" {
            0 => Unit,
            1 => Scalar(result: QualifiedScalar),
            2 => Structural(result: StructuralResult),
        }
    }

    struct OptionalCallable;
    impl Field<Option<CheckedBoundaryCallable>> for OptionalCallable {
        fn write(
            writer: &mut Writer,
            value: &Option<CheckedBoundaryCallable>,
        ) -> Result<(), CodecError> {
            writer.boolean(value.is_some());
            if let Some(callable) = value {
                Callable::write(writer, callable)?;
            }
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<Option<CheckedBoundaryCallable>, CodecError> {
            Ok(if reader.boolean()? {
                Some(Callable::read(reader)?)
            } else {
                None
            })
        }
    }

    struct NumericIntrinsic;
    impl Field<Option<language_semantics::numeric_intrinsics::NumericIntrinsic>> for NumericIntrinsic {
        fn write(
            writer: &mut Writer,
            value: &Option<language_semantics::numeric_intrinsics::NumericIntrinsic>,
        ) -> Result<(), CodecError> {
            writer.boolean(value.is_some());
            if let Some(meaning) = value {
                let Some(bytes) = meaning.canonical_encoding() else {
                    return crate::codec_error::malformed("unsupported numeric intrinsic meaning");
                };
                writer.bytes(&bytes);
            }
            Ok(())
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<Option<language_semantics::numeric_intrinsics::NumericIntrinsic>, CodecError>
        {
            if !reader.boolean()? {
                return Ok(None);
            }
            let mut bytes =
                [0; language_semantics::numeric_intrinsics::NumericIntrinsic::ENCODED_SIZE];
            for byte in &mut bytes {
                *byte = reader.u8()?;
            }
            language_semantics::numeric_intrinsics::NumericIntrinsic::from_canonical_encoding(bytes)
                .map(Some)
                .ok_or({
                    CodecError::MalformedStructuralFoundation("invalid numeric intrinsic meaning")
                })
        }
    }

    struct Completion;
    impl Field<BoundaryCompletion> for Completion {
        fn write(writer: &mut Writer, value: &BoundaryCompletion) -> Result<(), CodecError> {
            writer.u8(match value {
                BoundaryCompletion::NormalReturn => 0,
                BoundaryCompletion::External(ExternalCompletionKind::ProcessExit) => 1,
            });
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<BoundaryCompletion, CodecError> {
            match reader.u8()? {
                0 => Ok(BoundaryCompletion::NormalReturn),
                1 => Ok(BoundaryCompletion::External(
                    ExternalCompletionKind::ProcessExit,
                )),
                tag => Err(CodecError::InvalidTag("BoundaryCompletion", tag)),
            }
        }
    }

    struct ReferenceOutputs;
    impl Field<Vec<BoundaryReferenceOutput>> for ReferenceOutputs {
        fn write(
            writer: &mut Writer,
            value: &Vec<BoundaryReferenceOutput>,
        ) -> Result<(), CodecError> {
            reference_outputs::encode(writer, value)
        }
        fn read(reader: &mut Reader<'_>) -> Result<Vec<BoundaryReferenceOutput>, CodecError> {
            reference_outputs::decode(reader)
        }
    }

    pub(crate) fn encode_structural_parameters(
        writer: &mut Writer,
        parameters: &[StructuralParameterDeclaration],
    ) -> Result<(), CodecError> {
        writer.counted(
            "structural parameters",
            parameters,
            StructuralParameter::write,
        )
    }

    pub(crate) fn decode_structural_parameters(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<StructuralParameterDeclaration>, CodecError> {
        List::<StructuralParameter>::read(reader)
    }

    pub(crate) fn encode_service_ceiling(
        writer: &mut Writer,
        services: &[ServiceId],
    ) -> Result<(), CodecError> {
        writer.ids("published service ceiling", services)
    }

    pub(crate) fn encode_projected_qualifications(
        writer: &mut Writer,
        qualifications: &[StructuralPathQualification],
    ) -> Result<(), CodecError> {
        writer.counted(
            "projected structural parameter qualifications",
            qualifications,
            PathQualification::write,
        )
    }

    pub(crate) fn decode_projected_qualifications(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<StructuralPathQualification>, CodecError> {
        List::<PathQualification>::read(reader)
    }

    pub(crate) fn encode_call_result(
        writer: &mut Writer,
        result: &BoundaryMachineResult,
    ) -> Result<(), CodecError> {
        CallResult::write(writer, result)
    }

    pub(crate) fn decode_call_result(
        reader: &mut Reader<'_>,
    ) -> Result<BoundaryMachineResult, CodecError> {
        CallResult::read(reader)
    }

    pub(crate) fn encode_content_projection_expression(
        writer: &mut Writer,
        capacity: &ContentProjectionExpression,
    ) -> Result<(), CodecError> {
        match capacity {
            ContentProjectionExpression::IntervalSet(members) => {
                writer.u8(1);
                writer.len("program-local interval members", members.len())?;
                for (start, end) in members {
                    encode_capacity_scalar(writer, start)?;
                    encode_capacity_scalar(writer, end)?;
                }
            }
            ContentProjectionExpression::CountedQuantity(magnitude) => {
                writer.u8(2);
                encode_capacity_scalar(writer, magnitude)?;
            }
        }
        Ok(())
    }

    /// A capacity scalar is one postorder table (`crate::term_table`).
    fn encode_capacity_scalar(
        writer: &mut Writer,
        scalar: &ContentProjectionScalar,
    ) -> Result<(), CodecError> {
        encode_term_table(writer, scalar)
    }

    impl TableTerm for ContentProjectionScalar {
        const LABEL: &'static str = "capacity scalar";

        fn children<'term>(&'term self, children: &mut Vec<&'term Self>) {
            match self {
                ContentProjectionScalar::Successor(inner) => children.push(inner),
                ContentProjectionScalar::Add(left, right)
                | ContentProjectionScalar::Subtract(left, right)
                | ContentProjectionScalar::Multiply(left, right) => {
                    children.extend([left.as_ref(), right.as_ref()])
                }
                ContentProjectionScalar::SubjectField(_)
                | ContentProjectionScalar::RuntimeScalarEmbedding(_)
                | ContentProjectionScalar::Natural(_) => {}
            }
        }

        fn write_row(&self, writer: &mut Writer) -> Result<(), CodecError> {
            match self {
                ContentProjectionScalar::SubjectField(path) => {
                    writer.u8(1);
                    writer.strings("program-local capacity field path", path)?;
                }
                ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
                    writer.u8(2);
                    writer.strings("program-local capacity field path", path)?;
                }
                ContentProjectionScalar::Natural(value) => {
                    writer.u8(3);
                    writer.string("program-local natural", value)?;
                }
                ContentProjectionScalar::Successor(_) => writer.u8(4),
                ContentProjectionScalar::Add(_, _) => writer.u8(5),
                ContentProjectionScalar::Subtract(_, _) => writer.u8(6),
                ContentProjectionScalar::Multiply(_, _) => writer.u8(7),
            }
            Ok(())
        }

        fn read_row(
            reader: &mut Reader<'_>,
            children: &mut TableChildren<'_, Self>,
        ) -> Result<Self, CodecError> {
            let tag = reader.u8()?;
            let arity = match tag {
                1..=3 => 0,
                4 => 1,
                5..=7 => 2,
                tag => return Err(CodecError::InvalidTag("ContentProjectionScalar", tag)),
            };
            let leaf = match tag {
                1 => Some(ContentProjectionScalar::SubjectField(
                    reader.strings("program-local capacity field path")?,
                )),
                2 => Some(ContentProjectionScalar::RuntimeScalarEmbedding(
                    reader.strings("program-local capacity field path")?,
                )),
                3 => Some(ContentProjectionScalar::Natural(
                    reader.string("program-local natural")?,
                )),
                _ => None,
            };
            if let Some(leaf) = leaf {
                return Ok(leaf);
            }
            let mut operands = children.take(reader, arity)?.into_iter().map(Box::new);
            let mut operand = || {
                operands
                    .next()
                    .ok_or(CodecError::NonCanonicalTermTable(Self::LABEL))
            };
            Ok(match tag {
                4 => ContentProjectionScalar::Successor(operand()?),
                5 => ContentProjectionScalar::Add(operand()?, operand()?),
                6 => ContentProjectionScalar::Subtract(operand()?, operand()?),
                _ => ContentProjectionScalar::Multiply(operand()?, operand()?),
            })
        }
    }

    pub(crate) fn decode_content_projection_expression(
        reader: &mut Reader<'_>,
    ) -> Result<ContentProjectionExpression, CodecError> {
        match reader.u8()? {
            1 => Ok(ContentProjectionExpression::IntervalSet(decode_counted(
                reader,
                |reader| Ok((decode_term_table(reader)?, decode_term_table(reader)?)),
            )?)),
            2 => Ok(ContentProjectionExpression::CountedQuantity(
                decode_term_table(reader)?,
            )),
            tag => Err(CodecError::InvalidTag("ContentProjectionExpression", tag)),
        }
    }

    pub(crate) fn encode_structural_access(writer: &mut Writer, access: StructuralAccess) {
        writer.u8(match access {
            StructuralAccess::Owned => 1,
            StructuralAccess::SharedBorrow => 2,
            StructuralAccess::MutableBorrow => 3,
            StructuralAccess::WriteOnlyBorrow => 4,
        });
    }

    pub(crate) fn decode_structural_access(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralAccess, CodecError> {
        decode_structural_access_tag(reader.u8()?)
    }

    fn decode_structural_access_tag(tag: u8) -> Result<StructuralAccess, CodecError> {
        match tag {
            1 => Ok(StructuralAccess::Owned),
            2 => Ok(StructuralAccess::SharedBorrow),
            3 => Ok(StructuralAccess::MutableBorrow),
            4 => Ok(StructuralAccess::WriteOnlyBorrow),
            tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
        }
    }

    pub(crate) fn decode_optional_structural_access(
        reader: &mut Reader<'_>,
    ) -> Result<Option<StructuralAccess>, CodecError> {
        match reader.u8()? {
            0 => Ok(None),
            tag => Ok(Some(decode_structural_access_tag(tag)?)),
        }
    }

    struct ReferenceRegions;
    impl Field<Vec<terminal_psi::TerminalDynamicReferenceRegion>> for ReferenceRegions {
        fn write(
            writer: &mut Writer,
            value: &Vec<terminal_psi::TerminalDynamicReferenceRegion>,
        ) -> Result<(), CodecError> {
            super::module_wire::dynamic_dispatch_wire::encode_reference_regions(writer, value)
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<Vec<terminal_psi::TerminalDynamicReferenceRegion>, CodecError> {
            super::module_wire::dynamic_dispatch_wire::decode_reference_regions(reader)
        }
    }

    #[cfg(test)]
    mod execution_tag_tests {
        use super::*;

        #[test]
        fn release_relocation_and_interrupt_terminal_tags_do_not_alias() {
            for (value, tag) in [
                (CheckedBoundaryExecution::EmptyRunRelease, 4),
                (CheckedBoundaryExecution::AccessRelocate, 5),
                (CheckedBoundaryExecution::StaticInterruptInstall, 6),
                (CheckedBoundaryExecution::StaticInterruptContext, 7),
                (CheckedBoundaryExecution::StaticInterruptBeginCompletion, 8),
                (CheckedBoundaryExecution::StaticInterruptFinishCompletion, 9),
            ] {
                let mut writer = Writer::default();
                Execution::write(&mut writer, &value).unwrap();
                let bytes = writer.finish();
                assert_eq!(bytes, [tag]);
                let mut reader = Reader::new(&bytes);
                assert_eq!(Execution::read(&mut reader).unwrap(), value);
                assert_eq!(reader.remaining(), 0);
            }
            assert!(Execution::read(&mut Reader::new(&[])).is_err());
            assert!(Execution::read(&mut Reader::new(&[255])).is_err());
        }
    }
}

pub(crate) mod structural_type_wire {

    use terminal_psi::TerminalIdentityIndex;
    use terminal_psi::{
        ArrayExtent, LayoutApplicationId, StructuralCaseDeclaration, StructuralRecordStorage,
        StructuralTypeDeclaration, StructuralTypeShape,
    };

    use super::CodecError;
    use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
    use super::structural_field_wire::{
        decode_byte_sequence_carrier, decode_structural_field, decode_structural_field_type,
        encode_byte_sequence_carrier, encode_structural_field, encode_structural_field_type,
    };
    use super::structural_signature_wire::{decode_structural_access, encode_structural_access};
    use super::wire::{Reader, Writer};
    use crate::sections::semantic_module::wire::{bounded_count, decode_counted};
    use crate::term_wire::proposition_wire::{decode_proposition, encode_proposition};

    /// A packed record or a sum that declares `requires` clauses takes its
    /// own shape tag, followed after the shape by the counted clauses, so a
    /// declaration without clauses keeps the bytes it had before clauses
    /// had a slot.
    const PACKED_RECORD_WITH_REQUIRES: u8 = 17;
    const SUM_WITH_REQUIRES: u8 = 18;

    pub(crate) fn encode_structural_type(
        writer: &mut Writer,
        declaration: &StructuralTypeDeclaration,
    ) -> Result<(), CodecError> {
        writer.boolean(declaration.quotient_carrier.is_some());
        if let Some(carrier) = declaration.quotient_carrier {
            writer.id(carrier);
        }
        writer.id(declaration.id);
        writer.u32(declaration.identity.0);
        writer.boolean(declaration.owns_linear_claim);
        writer.boolean(declaration.no_code_affine_cleanup);
        writer.boolean(declaration.descriptor_cleanup);
        writer.boolean(declaration.copyable);
        match &declaration.shape {
            StructuralTypeShape::Opaque => {
                if declaration.quotient_carrier.is_some()
                    || declaration.scalar_restriction.is_some()
                    || !declaration.requires.is_empty()
                {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "opaque data cannot supply a visible carrier or authored value predicate",
                    ));
                }
                writer.u8(19);
            }
            StructuralTypeShape::Reference { referent, access } => {
                writer.u8(7);
                writer.id(*referent);
                encode_structural_access(writer, *access);
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                writer.u8(if declaration.scalar_restriction.is_some() {
                    12
                } else {
                    6
                });
                encode_scalar_type(writer, *scalar_type)?;
                if let Some(restriction) = &declaration.scalar_restriction {
                    encode_structural_field_type(writer, restriction)?;
                }
            }
            StructuralTypeShape::AtomicCell(cell) => {
                if !cell.is_supported() || declaration.scalar_restriction.is_some() {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "atomic storage has an invalid resident contract",
                    ));
                }
                writer.u8(16);
                encode_structural_field_type(writer, &(*cell).into())?;
            }
            StructuralTypeShape::ByteSequence(carrier) => {
                writer.u8(4);
                encode_byte_sequence_carrier(writer, *carrier);
            }
            StructuralTypeShape::ElementView { element, access } => {
                writer.u8(8);
                writer.id(*element);
                writer.boolean(access.is_some());
                if let Some(access) = access {
                    encode_structural_access(writer, *access);
                }
            }
            StructuralTypeShape::Record { fields, storage } => {
                writer.u8(match storage {
                    StructuralRecordStorage::Packed if !declaration.requires.is_empty() => {
                        PACKED_RECORD_WITH_REQUIRES
                    }
                    StructuralRecordStorage::Packed => 1,
                    StructuralRecordStorage::AccessView(_) => 14,
                    StructuralRecordStorage::Accessor(_) => 15,
                    StructuralRecordStorage::Authored(_) => 10,
                    StructuralRecordStorage::TargetLayout(_) => 13,
                });
                if let StructuralRecordStorage::TargetLayout(application) = storage {
                    writer.u32(application.0);
                }
                if let StructuralRecordStorage::Authored(storage)
                | StructuralRecordStorage::AccessView(storage)
                | StructuralRecordStorage::Accessor(storage) = storage
                {
                    writer.u64(storage.byte_size);
                    writer.u64(storage.byte_alignment);
                }
                writer.counted("structural fields", fields, encode_structural_field)?;
                if matches!(
                    storage,
                    StructuralRecordStorage::Authored(_)
                        | StructuralRecordStorage::AccessView(_)
                        | StructuralRecordStorage::Accessor(_)
                ) {
                    for field in fields {
                        encode_authored_field_storage(writer, field.storage.as_ref())?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                writer.u8(2);
                writer.id(*element);
                match length {
                    ArrayExtent::Literal(length) => {
                        writer.u8(0);
                        writer.u64(*length);
                    }
                    ArrayExtent::Application(application) => {
                        writer.u8(1);
                        writer.id(*application);
                    }
                }
            }
            StructuralTypeShape::Sum { cases } => {
                writer.u8(if declaration.requires.is_empty() {
                    3
                } else {
                    SUM_WITH_REQUIRES
                });
                writer.len("structural cases", cases.len())?;
                for case in cases {
                    writer.id(case.id);
                    writer.u32(case.identity.0);
                    writer.len("structural case payload fields", case.fields.len())?;
                    for field in &case.fields {
                        encode_structural_field(writer, field)?;
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                writer.u8(5);
                writer.counted("mixed structural fields", fields, encode_structural_field)?;
                writer.len("mixed structural cases", cases.len())?;
                for case in cases {
                    writer.id(case.id);
                    writer.u32(case.identity.0);
                    writer.len("mixed structural case payload fields", case.fields.len())?;
                    for field in &case.fields {
                        encode_structural_field(writer, field)?;
                    }
                }
            }
        }
        if !declaration.requires.is_empty() {
            if !matches!(
                declaration.shape,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    ..
                } | StructuralTypeShape::Sum { .. }
            ) {
                return Err(CodecError::MalformedStructuralFoundation(
                    "only a packed record or a sum declares requires clauses",
                ));
            }
            writer.len("structural type requires", declaration.requires.len())?;
            for clause in &declaration.requires {
                encode_proposition(writer, clause)?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode_structural_type(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralTypeDeclaration, CodecError> {
        let quotient_carrier = if reader.boolean()? {
            Some(reader.id("StructuralTypeId")?)
        } else {
            None
        };
        let id = reader.id("StructuralTypeId")?;
        let identity = TerminalIdentityIndex(reader.u32()?);
        let owns_linear_claim = reader.boolean()?;
        let no_code_affine_cleanup = reader.boolean()?;
        let descriptor_cleanup = reader.boolean()?;
        let copyable = reader.boolean()?;
        let mut scalar_restriction = None;
        let tag = reader.u8()?;
        let declares_requires = matches!(tag, PACKED_RECORD_WITH_REQUIRES | SUM_WITH_REQUIRES);
        let shape = match tag {
            19 => StructuralTypeShape::Opaque,
            7 => StructuralTypeShape::Reference {
                referent: reader.id("StructuralTypeId")?,
                access: decode_structural_access(reader)?,
            },
            6 => StructuralTypeShape::PrimitiveScalar(decode_scalar_type(reader)?),
            16 => {
                let cell = decode_structural_field_type(reader)?
                    .atomic_cell_type()
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "atomic storage has an invalid resident contract",
                    ))?;
                StructuralTypeShape::AtomicCell(cell)
            }
            12 => {
                let carrier = decode_scalar_type(reader)?;
                scalar_restriction = Some(decode_structural_field_type(reader)?);
                StructuralTypeShape::PrimitiveScalar(carrier)
            }
            tag @ (14 | 15) => {
                let geometry = semantic_vocabulary::AuthoredRecordStorage {
                    byte_size: reader.u64()?,
                    byte_alignment: reader.u64()?,
                };
                let mut fields = decode_counted(reader, decode_structural_field)?;
                for field in &mut fields {
                    field.storage = decode_authored_field_storage(reader)?;
                }
                StructuralTypeShape::Record {
                    fields,
                    storage: if tag == 14 {
                        StructuralRecordStorage::AccessView(geometry)
                    } else {
                        StructuralRecordStorage::Accessor(geometry)
                    },
                }
            }
            1 | PACKED_RECORD_WITH_REQUIRES => StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: decode_counted(reader, decode_structural_field)?,
            },
            10 => {
                let storage = semantic_vocabulary::AuthoredRecordStorage {
                    byte_size: reader.u64()?,
                    byte_alignment: reader.u64()?,
                };
                let mut fields = decode_counted(reader, decode_structural_field)?;
                for field in &mut fields {
                    field.storage = decode_authored_field_storage(reader)?;
                }
                StructuralTypeShape::Record {
                    fields,
                    storage: StructuralRecordStorage::Authored(storage),
                }
            }
            13 => StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(LayoutApplicationId(reader.u32()?)),
                fields: decode_counted(reader, decode_structural_field)?,
            },
            8 => StructuralTypeShape::ElementView {
                element: reader.id("StructuralTypeId")?,
                access: if reader.boolean()? {
                    Some(decode_structural_access(reader)?)
                } else {
                    None
                },
            },
            2 => StructuralTypeShape::FixedArray {
                element: reader.id("StructuralTypeId")?,
                length: match reader.u8()? {
                    0 => ArrayExtent::Literal(reader.u64()?),
                    1 => ArrayExtent::Application(reader.id("ScalarApplicationId")?),
                    tag => return Err(CodecError::InvalidTag("ArrayExtent", tag)),
                },
            },
            3 | SUM_WITH_REQUIRES => StructuralTypeShape::Sum {
                cases: decode_counted(reader, |reader| {
                    Ok(StructuralCaseDeclaration {
                        id: reader.id("StructuralCaseId")?,
                        identity: TerminalIdentityIndex(reader.u32()?),
                        fields: decode_counted(reader, decode_structural_field)?,
                    })
                })?,
            },
            4 => StructuralTypeShape::ByteSequence(decode_byte_sequence_carrier(reader)?),
            5 => StructuralTypeShape::Mixed {
                fields: decode_counted(reader, decode_structural_field)?,
                cases: decode_counted(reader, |reader| {
                    Ok(StructuralCaseDeclaration {
                        id: reader.id("StructuralCaseId")?,
                        identity: TerminalIdentityIndex(reader.u32()?),
                        fields: decode_counted(reader, decode_structural_field)?,
                    })
                })?,
            },
            tag => return Err(CodecError::InvalidTag("StructuralTypeShape", tag)),
        };
        let requires = if declares_requires {
            let requires = decode_counted(reader, decode_proposition)?;
            if requires.is_empty() {
                // The tags without clauses already say "none"; a second
                // spelling of no clauses would give one declaration two
                // encodings.
                return Err(CodecError::MalformedStructuralFoundation(
                    "a declaration tagged with requires clauses declares none",
                ));
            }
            requires
        } else {
            Vec::new()
        };
        Ok(StructuralTypeDeclaration {
            requires,
            quotient_carrier,
            owns_linear_claim,
            no_code_affine_cleanup,
            copyable,
            descriptor_cleanup,
            scalar_restriction,
            id,
            identity,
            shape,
        })
    }

    pub(crate) fn encode_authored_field_storage(
        writer: &mut Writer,
        storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
    ) -> Result<(), CodecError> {
        use semantic_vocabulary::AuthoredFieldStorage;
        match storage {
            None => writer.u8(0),
            Some(AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar,
            }) => {
                writer.u8(if stored_scalar.is_some() { 2 } else { 1 });
                writer.u64(*byte_offset);
                if let Some(scalar) = stored_scalar {
                    encode_scalar_type(writer, *scalar)?;
                }
            }
            Some(AuthoredFieldStorage::Repeated {
                byte_offset,
                element_stride,
            }) => {
                writer.u8(3);
                writer.u64(*byte_offset);
                writer.u64(*element_stride);
            }
            Some(AuthoredFieldStorage::Direct { byte_offset }) => {
                writer.u8(4);
                writer.u64(*byte_offset);
            }
            Some(AuthoredFieldStorage::Fragmented(recipe)) => {
                writer.u8(6);
                writer.len("authored bit fragments", recipe.fragments().len())?;
                for fragment in recipe.fragments() {
                    writer.u64(fragment.container_byte_offset);
                    writer.u64(fragment.container_width_bits);
                    writer.u64(fragment.destination_lsb);
                    writer.u64(fragment.source_lsb);
                    writer.u64(fragment.width);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn decode_authored_field_storage(
        reader: &mut Reader<'_>,
    ) -> Result<Option<semantic_vocabulary::AuthoredFieldStorage>, CodecError> {
        use semantic_vocabulary::AuthoredFieldStorage;
        Ok(match reader.u8()? {
            0 => None,
            1 => Some(AuthoredFieldStorage::Scalar {
                byte_offset: reader.u64()?,
                stored_scalar: None,
            }),
            2 => Some(AuthoredFieldStorage::Scalar {
                byte_offset: reader.u64()?,
                stored_scalar: Some(decode_scalar_type(reader)?),
            }),
            3 => Some(AuthoredFieldStorage::Repeated {
                byte_offset: reader.u64()?,
                element_stride: reader.u64()?,
            }),
            4 => Some(AuthoredFieldStorage::Direct {
                byte_offset: reader.u64()?,
            }),
            6 => {
                let count = bounded_count(reader, 40).map_err(|_| {
                    CodecError::MalformedStructuralFoundation(
                        "fragmented field requires a complete recipe",
                    )
                })?;
                if count == 0 {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "fragmented field requires a complete recipe",
                    ));
                }
                let mut fragments = Vec::with_capacity(count);
                for _ in 0..count {
                    fragments.push(semantic_vocabulary::AuthoredBitFragment {
                        container_byte_offset: reader.u64()?,
                        container_width_bits: reader.u64()?,
                        destination_lsb: reader.u64()?,
                        source_lsb: reader.u64()?,
                        width: reader.u64()?,
                    });
                }
                Some(AuthoredFieldStorage::Fragmented(
                    semantic_vocabulary::AuthoredBitFragments::new(fragments),
                ))
            }
            tag => return Err(CodecError::InvalidTag("AuthoredFieldStorage", tag)),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{
            AuthoredFieldStorage, AuthoredRecordStorage, IntegerSign, IntegerType, ScalarType,
            StructuralFieldId, StructuralTypeId,
        };
        use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};

        #[test]
        fn logical_copy_permission_changes_encoded_identity() {
            let mut encodings = Vec::new();
            for copyable in [false, true] {
                let declaration = StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: true,
                    copyable,
                    scalar_restriction: None,
                    id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::Record {
                        fields: Vec::new(),
                        storage: terminal_psi::StructuralRecordStorage::Packed,
                    },
                };
                let mut writer = Writer::default();
                encode_structural_type(&mut writer, &declaration).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_structural_type(&mut reader).unwrap(), declaration);
                assert_eq!(reader.remaining(), 0);
                assert!(!encodings.contains(&bytes));
                encodings.push(bytes);
            }
            let mut old = super::super::MAGIC.to_vec();
            old.extend_from_slice(&183_u16.to_le_bytes());
            assert!(matches!(
                super::super::decode_module(&old),
                Err(CodecError::UnsupportedFormatMarker(183))
            ));
        }

        #[test]
        fn declaration_owned_linearity_round_trips_and_changes_bytes() {
            let mut encodings = Vec::new();
            for owns_linear_claim in [false, true] {
                let declaration = StructuralTypeDeclaration {
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim,
                    no_code_affine_cleanup: false,
                    copyable: false,
                    descriptor_cleanup: false,
                    scalar_restriction: None,
                    id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(1),
                    shape: StructuralTypeShape::Record {
                        fields: Vec::new(),
                        storage: terminal_psi::StructuralRecordStorage::Packed,
                    },
                };
                let mut writer = Writer::default();
                encode_structural_type(&mut writer, &declaration).unwrap();
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(decode_structural_type(&mut reader).unwrap(), declaration);
                assert_eq!(reader.remaining(), 0);
                assert!(!encodings.contains(&bytes));
                encodings.push(bytes);
            }
        }

        #[test]
        fn declaration_owned_linearity_rejects_the_previous_module_schema() {
            let mut bytes = super::super::MAGIC.to_vec();
            bytes.extend_from_slice(&167_u16.to_le_bytes());
            assert!(matches!(
                super::super::decode_module(&bytes),
                Err(CodecError::UnsupportedFormatMarker(167))
            ));
        }

        #[test]
        fn authored_record_geometry_round_trips_without_widening_its_scalar() {
            let stored =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("signed byte"));
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: false,
                descriptor_cleanup: true,
                scalar_restriction: None,
                id: StructuralTypeId::new(1).expect("record identity"),
                identity: terminal_psi::TerminalIdentityIndex(15),
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 1,
                        byte_alignment: 1,
                    }),
                    fields: vec![StructuralFieldDeclaration {
                        inline_carrier: None,
                        id: StructuralFieldId::new(1).expect("field identity"),
                        identity: terminal_psi::TerminalIdentityIndex(16),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 64).expect("logical integer"),
                        )),
                        storage: Some(AuthoredFieldStorage::Scalar {
                            byte_offset: 0,
                            stored_scalar: Some(stored),
                        }),
                    }],
                },
            };
            let mut writer = Writer::default();
            encode_structural_type(&mut writer, &declaration).expect("encode record");
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                decode_structural_type(&mut reader).expect("decode record"),
                declaration
            );
            assert_eq!(reader.remaining(), 0);
        }

        #[test]
        fn authored_field_wire_preserves_absence_stride_and_fragmentation() {
            for storage in [
                None,
                Some(semantic_vocabulary::AuthoredFieldStorage::Repeated {
                    byte_offset: 3,
                    element_stride: 5,
                }),
                Some(semantic_vocabulary::AuthoredFieldStorage::Direct { byte_offset: 7 }),
                Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(
                    semantic_vocabulary::AuthoredBitFragments::new([
                        semantic_vocabulary::AuthoredBitFragment {
                            container_byte_offset: 7,
                            container_width_bits: 16,
                            destination_lsb: 4,
                            source_lsb: 0,
                            width: 3,
                        },
                        semantic_vocabulary::AuthoredBitFragment {
                            container_byte_offset: 9,
                            container_width_bits: 8,
                            destination_lsb: 1,
                            source_lsb: 3,
                            width: 2,
                        },
                    ]),
                )),
            ] {
                let mut writer = Writer::default();
                encode_authored_field_storage(&mut writer, storage.as_ref())
                    .expect("encode field geometry");
                let bytes = writer.finish();
                let mut reader = Reader::new(&bytes);
                assert_eq!(
                    decode_authored_field_storage(&mut reader).expect("decode field geometry"),
                    storage
                );
                assert_eq!(reader.remaining(), 0);
            }
        }
    }
}
