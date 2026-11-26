/*
Canonical wire encoders and decoders for structural type declarations, fields, places, paths,
operation results, and machine structural signatures.
*/

pub(crate) use crate::semantic_module::*;

pub(crate) mod structural_field_wire {
    use semantic_vocabulary::{BoundedIntegerType, ScalarRefinement};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet,
        StructuralFieldDeclaration, StructuralFieldType, TerminalIdentityIndex,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        Field, FloatFormatField, Id, Index, Integer, IntegerTypeField, OptionalAccess, OptionalId,
        Proposition, Scalar, U64, wire_enum,
    };

    pub(crate) use psi_declaration_encoding::structural_fields::*;

    wire_enum! {
        Carrier: ByteSequenceCarrier, "ByteSequenceCarrier" {
            1 => BorrowedView { access: OptionalAccess },
            2 => BoundedOwned { capacity: U64 },
            3 => BoundedView { access: OptionalAccess, capacity: U64 },
        }
        Relevance: BindingRelevance, "BindingRelevance" {
            1 => Relevant,
            2 => Erased,
        }
    }

    #[cfg(test)]
    pub(crate) fn decode_byte_sequence_carrier(
        reader: &mut Reader<'_>,
    ) -> Result<ByteSequenceCarrier, CodecError> {
        Carrier::read(reader)
    }

    struct BoundedInt;
    impl Field<BoundedIntegerType> for BoundedInt {
        fn write(writer: &mut Writer, value: &BoundedIntegerType) -> Result<(), CodecError> {
            IntegerTypeField::write(writer, &value.integer_type())?;
            Integer::write(writer, &value.minimum())?;
            Integer::write(writer, &value.maximum())
        }
        fn read(reader: &mut Reader<'_>) -> Result<BoundedIntegerType, CodecError> {
            BoundedIntegerType::new(
                IntegerTypeField::read(reader)?,
                Integer::read(reader)?,
                Integer::read(reader)?,
            )
            .map_err(CodecError::MalformedProposition)
        }
    }

    pub(crate) fn encode_structural_field(
        writer: &mut Writer,
        field: &StructuralFieldDeclaration,
    ) -> Result<(), CodecError> {
        Id::write(writer, &field.id)?;
        Index::write(writer, &field.identity)?;
        Index::write(writer, &field.declaration_identity)?;
        Relevance::write(writer, &field.relevance)?;
        OptionalId::write(writer, &field.inline_carrier)?;
        encode_structural_field_type(writer, &field.field_type)
    }

    pub(crate) fn decode_structural_field(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralFieldDeclaration, CodecError> {
        Ok(StructuralFieldDeclaration {
            id: Id::read(reader)?,
            identity: Index::read(reader)?,
            declaration_identity: Index::read(reader)?,
            relevance: Relevance::read(reader)?,
            inline_carrier: OptionalId::read(reader)?,
            field_type: decode_structural_field_type(reader)?,
            storage: None,
        })
    }

    pub(crate) fn encode_structural_field_type(
        writer: &mut Writer,
        field_type: &StructuralFieldType,
    ) -> Result<(), CodecError> {
        match field_type {
            StructuralFieldType::Scalar(s) => {
                writer.u8(1);
                Scalar::write(writer, s)
            }
            StructuralFieldType::Structural(id) => {
                writer.u8(2);
                Id::write(writer, id)
            }
            StructuralFieldType::Erased {
                type_identity,
                scalar: None,
            } => {
                writer.u8(3);
                Index::write(writer, type_identity)
            }
            StructuralFieldType::IeeeFloat(f) => {
                writer.u8(4);
                FloatFormatField::write(writer, f)
            }
            StructuralFieldType::ByteSequence(c) => {
                writer.u8(5);
                Carrier::write(writer, c)
            }
            StructuralFieldType::BoundedInteger(b) => {
                writer.u8(6);
                BoundedInt::write(writer, b)
            }
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            } => {
                writer.u8(7);
                Carrier::write(writer, carrier)?;
                writer.u8(predicates.bits());
                Ok(())
            }
            StructuralFieldType::Atomic(s) => {
                writer.u8(8);
                Scalar::write(writer, s)
            }
            StructuralFieldType::RefinedScalar(r) => {
                writer.u8(9);
                Scalar::write(writer, &r.carrier())?;
                Proposition::write(writer, r.predicate())
            }
            StructuralFieldType::AtomicBoundedInteger(b) => {
                writer.u8(10);
                BoundedInt::write(writer, b)
            }
            StructuralFieldType::Erased {
                type_identity,
                scalar: Some(s),
            } => {
                writer.u8(11);
                Index::write(writer, type_identity)?;
                Scalar::write(writer, s)
            }
        }
    }

    pub(crate) fn decode_structural_field_type(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralFieldType, CodecError> {
        Ok(match reader.u8()? {
            1 => StructuralFieldType::Scalar(Scalar::read(reader)?),
            2 => StructuralFieldType::Structural(Id::read(reader)?),
            3 => StructuralFieldType::Erased {
                type_identity: TerminalIdentityIndex(reader.u32()?),
                scalar: None,
            },
            4 => StructuralFieldType::IeeeFloat(FloatFormatField::read(reader)?),
            5 => StructuralFieldType::ByteSequence(Carrier::read(reader)?),
            6 => StructuralFieldType::BoundedInteger(BoundedInt::read(reader)?),
            7 => {
                let carrier = Carrier::read(reader)?;
                let bits = reader.u8()?;
                let predicates = ByteSequencePredicateSet::from_bits(bits)
                    .filter(|predicates| !predicates.is_empty())
                    .ok_or(CodecError::InvalidTag("ByteSequencePredicateSet", bits))?;
                StructuralFieldType::PredicateByteSequence {
                    carrier,
                    predicates,
                }
            }
            8 => StructuralFieldType::Atomic(Scalar::read(reader)?),
            9 => StructuralFieldType::RefinedScalar(
                ScalarRefinement::new(Scalar::read(reader)?, Proposition::read(reader)?).map_err(
                    |_| {
                        CodecError::MalformedStructuralFoundation(
                            "scalar field predicate is not closed and carrier-typed",
                        )
                    },
                )?,
            ),
            10 => StructuralFieldType::AtomicBoundedInteger(BoundedInt::read(reader)?),
            11 => StructuralFieldType::Erased {
                type_identity: TerminalIdentityIndex(reader.u32()?),
                scalar: Some(Scalar::read(reader)?),
            },
            tag => return Err(CodecError::InvalidTag("StructuralFieldType", tag)),
        })
    }

    #[cfg(test)]
    mod reference_path_wire_tests {
        use super::super::scalar_term_wire::{decode_scalar_term, encode_scalar_term};
        use semantic_vocabulary::{
            CanonicalStructuralPathSegment, PlaceId, ScalarReadProjection, ScalarTerm, ScalarType,
            StructuralFieldId,
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
    use crate::sections::semantic_module::wire::{Reader, Writer};
    use crate::wire_fields::{
        Argument, ClaimTransfers, Field, Id, Ids, List, Path, wire_enum, wire_record,
    };

    use terminal_psi::{
        OwnedCleanupInvocation, StructuralAffineDiscard, StructuralArgument,
        TerminalAffineCleanupAction,
    };

    pub(crate) use psi_declaration_encoding::places::*;

    // Borrowed-cleanup tags 3 and 5 remain retired. An owned call keeps its
    // exact source, access, path and claim correspondence.
    wire_record! {
        ResidualDiscard: StructuralAffineDiscard {
            place: Id,
            path: Path,
            structural_type: Id,
        }
        OwnedCleanup: OwnedCleanupInvocation {
            subject: Argument,
            structural_type: Id,
            cleanup_machine: Id,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
        }
    }

    wire_enum! {
        Cleanup: TerminalAffineCleanupAction, "TerminalAffineCleanupAction" {
            1 => DiscardRoot(place: Id),
            2 => DiscardResidual(discard: ResidualDiscard),
            4 => RetireHome(place: Id),
            6 => InvokeOwned(cleanup: OwnedCleanup),
        }
    }

    pub(crate) fn encode_structural_arguments(
        writer: &mut Writer,
        arguments: &[StructuralArgument],
    ) -> Result<(), CodecError> {
        writer.counted("structural arguments", arguments, Argument::write)
    }

    pub(crate) fn decode_structural_arguments(
        reader: &mut Reader<'_>,
    ) -> Result<Vec<StructuralArgument>, CodecError> {
        List::<Argument>::read(reader)
    }

    pub(crate) fn encode_affine_cleanup_action(
        writer: &mut Writer,
        action: &TerminalAffineCleanupAction,
    ) -> Result<(), CodecError> {
        Cleanup::write(writer, action)
    }

    pub(crate) fn decode_affine_cleanup_action(
        reader: &mut Reader<'_>,
    ) -> Result<TerminalAffineCleanupAction, CodecError> {
        Cleanup::read(reader)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use semantic_vocabulary::{ClaimId, MachineId, ObligationId, PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};

        #[test]
        fn declared_lifetime_families_round_trip_without_becoming_operand_paths() {
            use super::super::module_wire::closed_conformance_wire::ResultLifetime;
            use super::super::wire_fields::Field;
            use semantic_vocabulary::StructuralCaseId;
            use terminal_psi::{
                BoundaryLifetimeDependency, DeclaredResultLifetime, StructuralAccess,
            };

            let path = vec![
                StructuralPathSegment::Field(TerminalIdentityIndex(3)),
                StructuralPathSegment::Case(StructuralCaseId::new(1).unwrap()),
                StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                StructuralPathSegment::AllElements,
                StructuralPathSegment::Field(TerminalIdentityIndex(5)),
            ];
            let lifetime = DeclaredResultLifetime {
                inputs: vec![BoundaryLifetimeDependency::Stored {
                    argument_index: 0,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                }],
                results: vec![path.clone()],
            };
            let mut writer = Writer::default();
            ResultLifetime::write(&mut writer, &lifetime).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(ResultLifetime::read(&mut reader).unwrap(), lifetime);
            assert_eq!(reader.remaining(), 0);

            let mut writer = Writer::default();
            encode_declared_lifetime_path(&mut writer, &path).unwrap();
            let family_bytes = writer.finish();
            let concrete_refusal = CodecError::MalformedStructuralFoundation(
                "returned-origin interpretation is not an operand or owned path",
            );
            assert_eq!(
                encode_structural_path(&mut Writer::default(), "operand", &path),
                Err(concrete_refusal.clone())
            );
            assert_eq!(
                decode_structural_path(&mut Reader::new(&family_bytes)),
                Err(concrete_refusal)
            );
            let backing = [terminal_psi::ReferenceOriginProjection::Exact(
                StructuralPathSegment::AllElements,
            )];
            let backing_refusal = CodecError::MalformedStructuralFoundation(
                "qualification wildcard is not a reference origin",
            );
            assert_eq!(
                encode_reference_origin_path(&mut Writer::default(), "backing", &backing),
                Err(backing_refusal.clone())
            );
            assert_eq!(
                decode_reference_origin_path(&mut Reader::new(&family_bytes)),
                Err(backing_refusal)
            );

            let dynamic = [StructuralPathSegment::FixedRange { start: 0, end: 1 }];
            let family_refusal = CodecError::MalformedStructuralFoundation(
                "declared lifetime path is not a static family",
            );
            assert_eq!(
                encode_declared_lifetime_path(&mut Writer::default(), &dynamic),
                Err(family_refusal.clone())
            );
            let mut writer = Writer::default();
            encode_structural_path(&mut writer, "operand", &dynamic).unwrap();
            let dynamic_bytes = writer.finish();
            assert_eq!(
                decode_declared_lifetime_path(&mut Reader::new(&dynamic_bytes)),
                Err(family_refusal)
            );
        }

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
        fn owned_cleanup_round_trip_retains_source_path_transfers_and_proofs() {
            let mut encodings = Vec::new();
            for path in [
                Vec::new(),
                vec![StructuralPathSegment::Field(TerminalIdentityIndex(14))],
                vec![
                    StructuralPathSegment::Field(TerminalIdentityIndex(14)),
                    StructuralPathSegment::FixedIndex(1),
                ],
            ] {
                let action = TerminalAffineCleanupAction::InvokeOwned(OwnedCleanupInvocation {
                    subject: StructuralArgument {
                        place: PlaceId::new(1).unwrap(),
                        access: terminal_psi::StructuralAccess::Owned,
                        path,
                    },
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    cleanup_machine: MachineId::new(3).unwrap(),
                    claim_transfers: vec![terminal_psi::ClaimTransfer {
                        claim: ClaimId::new(4).unwrap(),
                        argument_index: 0,
                        path: vec![StructuralPathSegment::FixedIndex(2)],
                    }],
                    requirement_obligations: vec![ObligationId::new(6).unwrap()],
                });
                let mut writer = Writer::default();
                encode_affine_cleanup_action(&mut writer, &action).unwrap();
                let encoded = writer.finish();
                assert_eq!(encoded[0], 6);
                let mut reader = Reader::new(&encoded);
                assert_eq!(decode_affine_cleanup_action(&mut reader).unwrap(), action);
                assert_eq!(reader.remaining(), 0);
                assert!(!encodings.contains(&encoded));
                encodings.push(encoded);
            }
        }

        #[test]
        fn borrowed_cleanup_tags_are_retired() {
            for tag in [3, 5] {
                assert_eq!(
                    decode_affine_cleanup_action(&mut Reader::new(&[tag])),
                    Err(CodecError::InvalidTag("TerminalAffineCleanupAction", tag)),
                );
            }
        }

        #[test]
        fn owned_cleanup_wire_change_rejects_previous_module_version() {
            for previous in [275_u16, 276] {
                let mut encoded = super::super::MAGIC.to_vec();
                crate::codec_bytes::push_encoded(&mut encoded, previous.to_le_bytes());
                assert!(matches!(
                    super::super::decode_module(&encoded),
                    Err(CodecError::UnsupportedFormatMarker(marker)) if marker == previous
                ));
            }
        }
    }
}

pub(crate) mod structural_result_wire {

    use terminal_psi::{
        ResultQualificationEstablishment, StructuralOperationResult, StructuralQualificationChange,
    };

    use super::CodecError;

    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        ClaimBinding, Field, Id, Ids, List, Multiplicity, PathQualification, U32, wire_record,
    };

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

    use psi_callable_encoding::results::validate_reference_source_order;

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
        QualificationChange: StructuralQualificationChange {
            machine: Id,
            edge: Id,
            argument_ordinal: U32,
            source: Id,
            destination: Id,
            establishments: List<Establishment>,
        }
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

    #[cfg(test)]
    pub(crate) use psi_declaration_encoding::results::decode_multiplicity;
}

pub(crate) mod structural_signature_wire {
    pub(crate) use psi_callable_encoding::signatures::{ParameterKind, StructuralParameter};
    #[cfg(test)]
    pub(crate) use psi_declaration_encoding::access::mode::decode_structural_access;
    pub(crate) use psi_declaration_encoding::access::mode::encode_structural_access;
    pub(crate) use psi_declaration_encoding::results::CallResult;

    use psi_declaration_encoding::access::mode::decode_structural_access_tag;
    use semantic_vocabulary::{ContentPlaceSegment, ContentPlaceVersion};
    use terminal_psi::{
        BoundaryCompletion, BoundaryContentGuarantee, BoundaryDeclarationRole,
        BoundaryMachineDeclaration, BoundaryRequirementKind, CheckedBoundaryCallable,
        CheckedBoundaryExecution, ExternalCompletionKind, ProgramLocalRootIntroductionSchema,
        RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
        RetainedBorrowPlaceRoot, ScalarDomainRequirement, StructuralAccess,
        StructuralDomainRequirement,
    };

    use super::CodecError;
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        Access, ContentAlgebra, ContentExpression, ContentProjection, CrashRoutes, Digest, Field,
        Flag, Id, Ids, Index, List, MachineDeclaration, Multiplicity, Nominal, Optional,
        OptionalId, Path, ProjectionIdentity, Proposition, QualificationPath, ReachArgument,
        Scalar, Text, U32, U64, delegated_fields, wire_enum, wire_record,
    };

    pub(crate) mod reference_outputs {
        use super::super::structural_place_wire::{decode_structural_path, encode_structural_path};
        use super::{
            Access, CodecError, Field, List, Path, Reader, U32, Writer, wire_enum, wire_record,
        };
        use terminal_psi::{
            BoundaryLifetimeDependency, BoundaryReferenceDestination, BoundaryReferenceOutput,
        };

        pub(crate) use psi_callable_encoding::signatures::{decode_dependency, encode_dependency};

        wire_enum! {
            Destination: BoundaryReferenceDestination, "BoundaryReferenceDestination" {
                0 => Parameter(position: U32),
            }
        }

        struct Dependency;
        impl Field<BoundaryLifetimeDependency> for Dependency {
            fn write(
                writer: &mut Writer,
                value: &BoundaryLifetimeDependency,
            ) -> Result<(), CodecError> {
                encode_dependency(writer, value, |writer, path| {
                    encode_structural_path(writer, "boundary reference lifetime path", path)
                })
            }
            fn read(reader: &mut Reader<'_>) -> Result<BoundaryLifetimeDependency, CodecError> {
                decode_dependency(reader, decode_structural_path)
            }
        }

        wire_record! {
            ReferenceOutput: BoundaryReferenceOutput {
                destination: Destination,
                path: Path,
                access: Access,
                dependencies: List<Dependency>,
            }
        }
    }

    type ReferenceOutputs = List<reference_outputs::ReferenceOutput>;

    wire_record! {
        BoundaryMachine: BoundaryMachineDeclaration {
            id: Id,
            declaration: Optional<Callable>,
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
            result_lifetimes: Optional<List<crate::module_wire::closed_conformance_wire::ResultLifetime>>,
            requires: List<DomainRequirement>,
            normal_qualifications: List<DomainRequirement>,
            membership_ensures: List<super::contract_wire::Membership>,
            scalar_domain_requirements: List<ScalarDomain>,
            scalar_requires: List<Proposition>,
            scalar_ensures: List<Proposition>,
            reference_outputs: ReferenceOutputs,
            program_local_root_introductions: List<RootIntroduction>,
            content_guarantees: List<ContentGuarantee>,
            fixed_service_reach: Ids,
            service_reach_is_installation_bound: Flag,
            owning_service: OptionalId,
            published_service_ceiling: Ids,
            published_synchronous_entries: Ids,
        }
        Callable: CheckedBoundaryCallable {
            service_declaration: Digest,
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
    }

    struct HostedOperationWire;
    impl Field<language_core::sealed_intrinsics::HostedOperation> for HostedOperationWire {
        fn write(
            writer: &mut Writer,
            value: &language_core::sealed_intrinsics::HostedOperation,
        ) -> Result<(), CodecError> {
            match value {
                language_core::sealed_intrinsics::HostedOperation::ProcessExit => {
                    writer.u8(0);
                    Ok(())
                }
                _ => Err(CodecError::InvalidTag("hosted primitive operation", 255)),
            }
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<language_core::sealed_intrinsics::HostedOperation, CodecError> {
            match reader.u8()? {
                0 => Ok(language_core::sealed_intrinsics::HostedOperation::ProcessExit),
                tag => Err(CodecError::InvalidTag("hosted primitive operation", tag)),
            }
        }
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
            10 => HostedPrimitive { operation: HostedOperationWire, requirement: Id },
            11 => NumericPrimitive,
            12 => LayoutTargetFacts,
        }
        Role: BoundaryDeclarationRole, "BoundaryDeclarationRole" { 0 => Callable, 1 => ContractEvidence }
        RequirementKind: BoundaryRequirementKind, "BoundaryRequirementKind" {
            0 => Machine,
            1 => Trait,
            2 => Operator,
        }
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
    }

    struct NumericIntrinsic;
    impl Field<Option<numerics::intrinsics::NumericIntrinsic>> for NumericIntrinsic {
        fn write(
            writer: &mut Writer,
            value: &Option<numerics::intrinsics::NumericIntrinsic>,
        ) -> Result<(), CodecError> {
            writer.boolean(value.is_some());
            if let Some(meaning) = value {
                let bytes = meaning.canonical_encoding().ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "unsupported numeric intrinsic meaning",
                    ),
                )?;
                writer.bytes(&bytes);
            }
            Ok(())
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<Option<numerics::intrinsics::NumericIntrinsic>, CodecError> {
            if !reader.boolean()? {
                return Ok(None);
            }
            let bytes =
                reader.array::<{ numerics::intrinsics::NumericIntrinsic::ENCODED_SIZE }>()?;
            numerics::intrinsics::NumericIntrinsic::from_canonical_encoding(bytes)
                .map(Some)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "invalid numeric intrinsic meaning",
                ))
        }
    }

    pub(crate) struct Completion;
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

    delegated_fields! {
        ReferenceRegions: Vec<terminal_psi::TerminalDynamicReferenceRegion>
            => super::module_wire::dynamic_dispatch_wire::encode_reference_regions,
               super::module_wire::dynamic_dispatch_wire::decode_reference_regions;
    }

    pub(crate) fn decode_optional_structural_access(
        reader: &mut Reader<'_>,
    ) -> Result<Option<StructuralAccess>, CodecError> {
        match reader.u8()? {
            0 => Ok(None),
            tag => Ok(Some(decode_structural_access_tag(tag)?)),
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
    use semantic_vocabulary::{
        AuthoredBitFragment, AuthoredBitFragments, AuthoredFieldStorage, AuthoredRecordStorage,
    };
    use terminal_psi::{
        ArrayExtent, LayoutApplicationId, NominalCleanupSelection, NominalDeclarationIdentity,
        StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralRecordStorage,
        StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    };

    use super::CodecError;
    use super::structural_field_wire::{
        Carrier, decode_structural_field, decode_structural_field_type, encode_structural_field,
        encode_structural_field_type,
    };
    use super::wire::{Reader, Writer};
    use super::wire_fields::{
        Access, Field, Flag, Id, Index, List, Multiplicity, Optional, Proposition, Scalar, U64,
        delegated_fields, wire_record,
    };
    use crate::sections::semantic_module::wire::bounded_count;

    /// A record keeps its invariant regardless of how its fields are laid
    /// out. Its existing requires wrapper encloses the ordinary storage shape;
    /// declarations without clauses keep their original shape encoding.
    const RECORD_WITH_REQUIRES: u8 = 17;
    const SUM_WITH_REQUIRES: u8 = 18;

    delegated_fields! {
        FieldDecl: StructuralFieldDeclaration => encode_structural_field, decode_structural_field;
    }

    wire_record! {
        RecordGeometry: AuthoredRecordStorage { byte_size: U64, byte_alignment: U64 }
        BitFragment: AuthoredBitFragment {
            container_byte_offset: U64,
            container_width_bits: U64,
            destination_lsb: U64,
            source_lsb: U64,
            width: U64,
        }
        CaseDecl: StructuralCaseDeclaration {
            declaration_identity: Index,
            id: Id,
            identity: Index,
            fields: List<FieldDecl>,
        }
    }

    struct NominalCleanup;
    impl Field<NominalCleanupSelection> for NominalCleanup {
        fn write(writer: &mut Writer, value: &NominalCleanupSelection) -> Result<(), CodecError> {
            writer.u32(value.declaration.identity().0);
            writer.id(value.cleanup_machine);
            Ok(())
        }
        fn read(reader: &mut Reader<'_>) -> Result<NominalCleanupSelection, CodecError> {
            Ok(NominalCleanupSelection {
                declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                cleanup_machine: reader.id("MachineId")?,
            })
        }
    }

    fn decode_authored_record_fields(
        reader: &mut Reader<'_>,
    ) -> Result<(AuthoredRecordStorage, Vec<StructuralFieldDeclaration>), CodecError> {
        let geometry = RecordGeometry::read(reader)?;
        let mut fields = List::<FieldDecl>::read(reader)?;
        for field in &mut fields {
            field.storage = decode_authored_field_storage(reader)?;
        }
        Ok((geometry, fields))
    }

    pub(crate) fn encode_structural_type(
        writer: &mut Writer,
        declaration: &StructuralTypeDeclaration,
    ) -> Result<(), CodecError> {
        Optional::<Id>::write(writer, &declaration.quotient_carrier)?;
        Id::write(writer, &declaration.id)?;
        Index::write(writer, &declaration.identity)?;
        Index::write(writer, &declaration.declaration_identity)?;
        Flag::write(writer, &declaration.owns_linear_claim)?;
        Flag::write(writer, &declaration.no_code_affine_cleanup)?;
        Flag::write(writer, &declaration.descriptor_cleanup)?;
        Multiplicity::write(writer, &declaration.multiplicity)?;
        Optional::<NominalCleanup>::write(writer, &declaration.nominal_cleanup)?;
        match &declaration.shape {
            StructuralTypeShape::TypeParameter => {
                return Err(CodecError::MalformedStructuralFoundation(
                    "an open type parameter cannot be serialized as executable storage",
                ));
            }
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
                Id::write(writer, referent)?;
                Access::write(writer, access)?;
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                writer.u8(if declaration.scalar_restriction.is_some() {
                    12
                } else {
                    6
                });
                Scalar::write(writer, scalar_type)?;
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
                Carrier::write(writer, carrier)?;
            }
            StructuralTypeShape::ElementView { element, access } => {
                writer.u8(8);
                Id::write(writer, element)?;
                Optional::<Access>::write(writer, access)?;
            }
            StructuralTypeShape::Record { fields, storage } => {
                if !declaration.requires.is_empty() {
                    writer.u8(RECORD_WITH_REQUIRES);
                }
                writer.u8(match storage {
                    StructuralRecordStorage::Packed => 1,
                    StructuralRecordStorage::Authored(_) => 10,
                    StructuralRecordStorage::TargetLayout(_) => 13,
                    StructuralRecordStorage::AccessView(_) => 14,
                    StructuralRecordStorage::Accessor(_) => 15,
                });
                let has_authored_fields = match storage {
                    StructuralRecordStorage::TargetLayout(application) => {
                        writer.u32(application.0);
                        false
                    }
                    StructuralRecordStorage::Authored(geometry)
                    | StructuralRecordStorage::AccessView(geometry)
                    | StructuralRecordStorage::Accessor(geometry) => {
                        RecordGeometry::write(writer, geometry)?;
                        true
                    }
                    StructuralRecordStorage::Packed => false,
                };
                writer.counted("structural fields", fields, encode_structural_field)?;
                if has_authored_fields {
                    for field in fields {
                        encode_authored_field_storage(writer, field.storage.as_ref())?;
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                writer.u8(2);
                Id::write(writer, element)?;
                match length {
                    ArrayExtent::Parameter { .. } => {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "an array extent is still a rigid const input",
                        ));
                    }
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
                List::<CaseDecl>::write(writer, cases)?;
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                writer.u8(5);
                List::<FieldDecl>::write(writer, fields)?;
                List::<CaseDecl>::write(writer, cases)?;
            }
        }
        if !declaration.requires.is_empty() {
            if !matches!(
                declaration.shape,
                StructuralTypeShape::Record { .. } | StructuralTypeShape::Sum { .. }
            ) {
                return Err(CodecError::MalformedStructuralFoundation(
                    "only a record or a sum declares requires clauses",
                ));
            }
            List::<Proposition>::write(writer, &declaration.requires)?;
        }
        Ok(())
    }

    pub(crate) fn decode_structural_type(
        reader: &mut Reader<'_>,
    ) -> Result<StructuralTypeDeclaration, CodecError> {
        let quotient_carrier = Optional::<Id>::read(reader)?;
        let id = Id::read(reader)?;
        let identity = Index::read(reader)?;
        let declaration_identity = Index::read(reader)?;
        let owns_linear_claim = Flag::read(reader)?;
        let no_code_affine_cleanup = Flag::read(reader)?;
        let descriptor_cleanup = Flag::read(reader)?;
        let multiplicity = Multiplicity::read(reader)?;
        let nominal_cleanup = Optional::<NominalCleanup>::read(reader)?;
        let mut scalar_restriction = None;
        let tag = reader.u8()?;
        let declares_requires = matches!(tag, RECORD_WITH_REQUIRES | SUM_WITH_REQUIRES);
        let shape_tag = if tag == RECORD_WITH_REQUIRES {
            let storage_tag = reader.u8()?;
            if !matches!(storage_tag, 1 | 10 | 13 | 14 | 15) {
                return Err(CodecError::MalformedStructuralFoundation(
                    "record requires wrapper must contain an ordinary record storage shape",
                ));
            }
            storage_tag
        } else {
            tag
        };
        let shape = match shape_tag {
            1 => StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: List::<FieldDecl>::read(reader)?,
            },
            2 => StructuralTypeShape::FixedArray {
                element: Id::read(reader)?,
                length: match reader.u8()? {
                    0 => ArrayExtent::Literal(reader.u64()?),
                    1 => ArrayExtent::Application(Id::read(reader)?),
                    tag => return Err(CodecError::InvalidTag("ArrayExtent", tag)),
                },
            },
            3 | SUM_WITH_REQUIRES => StructuralTypeShape::Sum {
                cases: List::<CaseDecl>::read(reader)?,
            },
            4 => StructuralTypeShape::ByteSequence(Carrier::read(reader)?),
            5 => StructuralTypeShape::Mixed {
                fields: List::<FieldDecl>::read(reader)?,
                cases: List::<CaseDecl>::read(reader)?,
            },
            6 => StructuralTypeShape::PrimitiveScalar(Scalar::read(reader)?),
            7 => StructuralTypeShape::Reference {
                referent: Id::read(reader)?,
                access: Access::read(reader)?,
            },
            8 => StructuralTypeShape::ElementView {
                element: Id::read(reader)?,
                access: Optional::<Access>::read(reader)?,
            },
            tag @ (10 | 14 | 15) => {
                let (geometry, fields) = decode_authored_record_fields(reader)?;
                let storage = match tag {
                    10 => StructuralRecordStorage::Authored(geometry),
                    14 => StructuralRecordStorage::AccessView(geometry),
                    _ => StructuralRecordStorage::Accessor(geometry),
                };
                StructuralTypeShape::Record { fields, storage }
            }
            12 => {
                let carrier = Scalar::read(reader)?;
                scalar_restriction = Some(decode_structural_field_type(reader)?);
                StructuralTypeShape::PrimitiveScalar(carrier)
            }
            13 => StructuralTypeShape::Record {
                storage: StructuralRecordStorage::TargetLayout(LayoutApplicationId(reader.u32()?)),
                fields: List::<FieldDecl>::read(reader)?,
            },
            16 => StructuralTypeShape::AtomicCell(
                decode_structural_field_type(reader)?
                    .atomic_cell_type()
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "atomic storage has an invalid resident contract",
                    ))?,
            ),
            19 => StructuralTypeShape::Opaque,
            tag => return Err(CodecError::InvalidTag("StructuralTypeShape", tag)),
        };
        let requires = if declares_requires {
            let requires = List::<Proposition>::read(reader)?;
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
            declaration_identity,
            requires,
            quotient_carrier,
            owns_linear_claim,
            no_code_affine_cleanup,
            multiplicity,
            descriptor_cleanup,
            nominal_cleanup,
            scalar_restriction,
            id,
            identity,
            shape,
        })
    }

    pub(crate) fn encode_authored_field_storage(
        writer: &mut Writer,
        storage: Option<&AuthoredFieldStorage>,
    ) -> Result<(), CodecError> {
        match storage {
            None => writer.u8(0),
            Some(AuthoredFieldStorage::Scalar {
                byte_offset,
                stored_scalar,
            }) => {
                writer.u8(if stored_scalar.is_some() { 2 } else { 1 });
                writer.u64(*byte_offset);
                if let Some(scalar) = stored_scalar {
                    Scalar::write(writer, scalar)?;
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
                writer.counted(
                    "authored bit fragments",
                    recipe.fragments(),
                    BitFragment::write,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn decode_authored_field_storage(
        reader: &mut Reader<'_>,
    ) -> Result<Option<AuthoredFieldStorage>, CodecError> {
        Ok(match reader.u8()? {
            0 => None,
            1 => Some(AuthoredFieldStorage::Scalar {
                byte_offset: reader.u64()?,
                stored_scalar: None,
            }),
            2 => Some(AuthoredFieldStorage::Scalar {
                byte_offset: reader.u64()?,
                stored_scalar: Some(Scalar::read(reader)?),
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
                    fragments.push(BitFragment::read(reader)?);
                }
                Some(AuthoredFieldStorage::Fragmented(AuthoredBitFragments::new(
                    fragments,
                )))
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

        fn invariant_record(storage: StructuralRecordStorage) -> StructuralTypeDeclaration {
            StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                requires: vec![
                    semantic_vocabulary::Proposition::Truth,
                    semantic_vocabulary::Proposition::Falsehood,
                ],
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                scalar_restriction: None,
                id: StructuralTypeId::new(1).unwrap(),
                identity: TerminalIdentityIndex(1),
                shape: StructuralTypeShape::Record {
                    storage,
                    fields: Vec::new(),
                },
            }
        }

        fn shape_offset(bytes: &[u8]) -> usize {
            let mut reader = Reader::new(bytes);
            assert!(!reader.boolean().unwrap());
            let _: StructuralTypeId = reader.id("StructuralTypeId").unwrap();
            reader.u32().unwrap();
            reader.u32().unwrap();
            for _ in 0..3 {
                reader.boolean().unwrap();
            }
            super::super::structural_result_wire::decode_multiplicity(&mut reader).unwrap();
            reader.boolean().unwrap();
            bytes.len() - reader.remaining()
        }

        #[test]
        fn selected_cleanup_retains_exact_declaration_and_application() {
            let mut encodings = Vec::new();
            for selected in [None, Some((2, 3)), Some((4, 3)), Some((2, 5))] {
                let mut declaration = invariant_record(StructuralRecordStorage::Packed);
                declaration.nominal_cleanup =
                    selected.map(
                        |(identity, machine)| terminal_psi::NominalCleanupSelection {
                            declaration: terminal_psi::NominalDeclarationIdentity::new(
                                TerminalIdentityIndex(identity),
                            ),
                            cleanup_machine: semantic_vocabulary::MachineId::new(machine).unwrap(),
                        },
                    );
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
        fn record_requires_preserve_the_storage_shape_and_ordered_clauses() {
            // Geometry and predicates are independent. Each existing storage
            // shape must keep its clauses in the same authored order.
            let geometry = AuthoredRecordStorage {
                byte_size: 0,
                byte_alignment: 1,
            };
            for storage in [
                StructuralRecordStorage::Packed,
                StructuralRecordStorage::Authored(geometry),
                StructuralRecordStorage::TargetLayout(LayoutApplicationId(1)),
                StructuralRecordStorage::AccessView(geometry),
                StructuralRecordStorage::Accessor(geometry),
            ] {
                let declaration = invariant_record(storage);
                let mut writer = Writer::default();
                encode_structural_type(&mut writer, &declaration).unwrap();
                let wrapped = writer.finish();
                let offset = shape_offset(&wrapped);
                assert_eq!(wrapped[offset], RECORD_WITH_REQUIRES);
                let mut reader = Reader::new(&wrapped);
                assert_eq!(decode_structural_type(&mut reader).unwrap(), declaration);
                assert_eq!(reader.remaining(), 0);

                let mut plain = declaration;
                plain.requires.clear();
                let mut writer = Writer::default();
                encode_structural_type(&mut writer, &plain).unwrap();
                let ordinary = writer.finish();
                assert_eq!(wrapped[..offset], ordinary[..offset]);
                assert_eq!(wrapped[offset + 1..ordinary.len() + 1], ordinary[offset..]);
            }

            let mut sum = invariant_record(StructuralRecordStorage::Packed);
            sum.declaration_identity = TerminalIdentityIndex(17);
            sum.shape = StructuralTypeShape::Sum {
                cases: vec![terminal_psi::StructuralCaseDeclaration {
                    declaration_identity: TerminalIdentityIndex(18),
                    id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(2),
                    fields: vec![StructuralFieldDeclaration {
                        declaration_identity: TerminalIdentityIndex(19),
                        inline_carrier: None,
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(3),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        storage: None,
                    }],
                }],
            };
            let mut writer = Writer::default();
            encode_structural_type(&mut writer, &sum).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[shape_offset(&bytes)], SUM_WITH_REQUIRES);
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_structural_type(&mut reader).unwrap(), sum);
            assert_eq!(reader.remaining(), 0);
        }

        #[test]
        fn record_requires_refuse_nonrecords_empty_clauses_and_truncation() {
            let declaration = invariant_record(StructuralRecordStorage::Packed);
            let mut writer = Writer::default();
            encode_structural_type(&mut writer, &declaration).unwrap();
            let bytes = writer.finish();
            let offset = shape_offset(&bytes);
            for tag in [RECORD_WITH_REQUIRES, SUM_WITH_REQUIRES, 2, 6, 19] {
                let mut wrong = bytes.clone();
                wrong[offset + 1] = tag;
                assert_eq!(
                    decode_structural_type(&mut Reader::new(&wrong)),
                    Err(CodecError::MalformedStructuralFoundation(
                        "record requires wrapper must contain an ordinary record storage shape"
                    ))
                );
            }
            for length in [offset, offset + 1, bytes.len() - 1] {
                assert!(decode_structural_type(&mut Reader::new(&bytes[..length])).is_err());
            }
            let mut plain = declaration;
            plain.requires.clear();
            let mut writer = Writer::default();
            encode_structural_type(&mut writer, &plain).unwrap();
            let mut empty = writer.finish();
            empty.insert(offset, RECORD_WITH_REQUIRES);
            let mut writer = Writer::default();
            writer.len("test empty clauses", 0).unwrap();
            empty.extend(writer.finish());
            assert_eq!(
                decode_structural_type(&mut Reader::new(&empty)),
                Err(CodecError::MalformedStructuralFoundation(
                    "a declaration tagged with requires clauses declares none"
                ))
            );
        }

        #[test]
        fn declared_multiplicity_round_trips_and_rejects_unknown_tags() {
            let mut encodings = Vec::new();
            for multiplicity in [
                terminal_psi::StructuralMultiplicity::Unrestricted,
                terminal_psi::StructuralMultiplicity::Affine,
                terminal_psi::StructuralMultiplicity::Linear,
            ] {
                let declaration = StructuralTypeDeclaration {
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim: false,
                    no_code_affine_cleanup: false,
                    descriptor_cleanup: true,
                    nominal_cleanup: None,
                    multiplicity,
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
                let position = shape_offset(&bytes) - 2;
                for tag in [0, 4, 255] {
                    let mut invalid = bytes.clone();
                    invalid[position] = tag;
                    assert!(matches!(decode_structural_type(&mut Reader::new(&invalid)),
                        Err(CodecError::InvalidTag("StructuralMultiplicity", found)) if found==tag));
                }
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
                    declaration_identity: Default::default(),
                    requires: Vec::new(),
                    quotient_carrier: None,
                    owns_linear_claim,
                    no_code_affine_cleanup: false,
                    multiplicity: terminal_psi::StructuralMultiplicity::Linear,
                    descriptor_cleanup: false,
                    nominal_cleanup: None,
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
                declaration_identity: Default::default(),
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                descriptor_cleanup: true,
                nominal_cleanup: None,
                scalar_restriction: None,
                id: StructuralTypeId::new(1).expect("record identity"),
                identity: terminal_psi::TerminalIdentityIndex(15),
                shape: StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                        byte_size: 1,
                        byte_alignment: 1,
                    }),
                    fields: vec![StructuralFieldDeclaration {
                        declaration_identity: Default::default(),
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
