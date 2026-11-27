/*
Canonical wire encoding and decoding for `TerminalMachine` control-flow blocks (`Block`,
`Operation`, `OperationKind`, `Terminator`), atomic and storage operations, call sites, and
terminator edges.
*/

pub(crate) mod atomic_operations {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{Field, Id, Path, wire_enum, wire_record};
    use language_core::atomic::{
        AtomicCompareExchangeAttemptAxis, AtomicCompareExchangeObservationAxis,
        AtomicCompareExchangeOnceResultCustody, AtomicCompareExchangeOutcomeIdentity,
    };
    use terminal_psi::{
        AtomicAccessEvent, AtomicReadModifyWrite, FenceOrdering, MemoryOrdering, OperationKind,
    };

    wire_enum! {
        ReadModifyWrite: AtomicReadModifyWrite, "AtomicReadModifyWrite" {
            1 => FetchAdd, 2 => FetchSub, 3 => FetchAnd, 4 => FetchOr, 5 => FetchXor,
        }
        AttemptAxis: AtomicCompareExchangeAttemptAxis, "AtomicCompareExchangeAttemptAxis" {
            1 => Decisive, 2 => SingleAttempt,
        }
        ObservationAxis: AtomicCompareExchangeObservationAxis, "AtomicCompareExchangeObservationAxis" {
            1 => Observing, 2 => NonObserving,
        }
        OutcomeIdentity: AtomicCompareExchangeOutcomeIdentity, "AtomicCompareExchangeOutcomeIdentity" {
            1 => AtomicCompareExchangeOutcome,
            2 => AtomicCompareExchangeOnceOutcome,
            3 => AtomicTryExchangeOutcome,
            4 => AtomicTryExchangeOnceOutcome,
        }
        Ordering: MemoryOrdering, "MemoryOrdering" {
            1 => NoOrdering, 2 => Receive, 3 => Publish, 4 => ReceivePublish, 5 => GlobalOrder,
        }
        Fence: FenceOrdering, "FenceOrdering" {
            1 => Receive, 2 => Publish, 3 => ReceivePublish,
        }
        Event: AtomicAccessEvent, "AtomicAccessEvent" {
            1 => Load { ordering: Ordering },
            2 => Store { value: Id, ordering: Ordering },
            3 => ReadModifyWrite { operation: ReadModifyWrite, operand: Id, ordering: Ordering },
            4 => Swap { value: Id, ordering: Ordering },
            5 => CompareExchange { expected: Id, replacement: Id, success: Ordering, failure: Ordering },
            6 => CompareExchangeOnce {
                custody: OnceCustody,
                expected: Id,
                replacement: Id,
                success: Ordering,
                failure: Ordering,
            },
        }
    }

    wire_record! {
        RawOnceCustody: AtomicCompareExchangeOnceResultCustody {
            attempt: AttemptAxis,
            observation: ObservationAxis,
            outcome_identity: OutcomeIdentity,
        }
    }

    pub(crate) struct OnceCustody;
    impl Field<AtomicCompareExchangeOnceResultCustody> for OnceCustody {
        fn write(
            writer: &mut Writer,
            custody: &AtomicCompareExchangeOnceResultCustody,
        ) -> Result<(), CodecError> {
            if !custody.is_canonical() {
                return crate::codec_error::malformed(
                    "atomic attempt has inconsistent result custody",
                );
            }
            RawOnceCustody::write(writer, custody)
        }
        fn read(
            reader: &mut Reader<'_>,
        ) -> Result<AtomicCompareExchangeOnceResultCustody, CodecError> {
            let custody = RawOnceCustody::read(reader)?;
            if !custody.is_canonical() {
                return crate::codec_error::malformed(
                    "atomic attempt has inconsistent result custody",
                );
            }
            Ok(custody)
        }
    }

    pub(crate) fn encode_atomic_access(
        writer: &mut Writer,
        place: semantic_vocabulary::PlaceId,
        path: &Vec<terminal_psi::StructuralPathSegment>,
        event: AtomicAccessEvent,
        resident_obligation: Option<semantic_vocabulary::ObligationId>,
    ) -> Result<(), CodecError> {
        writer.u8(if resident_obligation.is_some() {
            131
        } else {
            93
        });
        if let Some(obligation) = resident_obligation {
            writer.id(obligation);
        }
        writer.id(place);
        Path::<false>::write(writer, path)?;
        Event::write(writer, &event)
    }

    pub(crate) fn decode_atomic_access(
        reader: &mut Reader<'_>,
        restricted: bool,
    ) -> Result<OperationKind, CodecError> {
        Ok(OperationKind::AtomicAccess {
            resident_obligation: restricted.then(|| reader.id("ObligationId")).transpose()?,
            place: reader.id("PlaceId")?,
            path: Path::<false>::read(reader)?,
            event: Event::read(reader)?,
        })
    }
}

pub(crate) mod call_operations {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use semantic_vocabulary::ServiceId;
    use terminal_psi::{OperationKind, RegisterAccessEvent, RegisterOperation};

    pub(crate) fn encode_register_access(
        writer: &mut Writer,
        service: Option<ServiceId>,
        event: RegisterAccessEvent,
    ) -> Result<(), CodecError> {
        if !event.operation().is_admitted()
            || service.is_some() != event.operation().requires_service_authority()
        {
            return crate::codec_error::malformed(
                "register access disagrees with its catalog service authority",
            );
        }
        if let Some(service) = service {
            writer.u8(121);
            writer.id(service);
        } else {
            writer.u8(126);
        }
        writer.u16(event.operation().catalog_tag());
        for value in event.inputs() {
            writer.id(*value);
        }
        Ok(())
    }

    pub(crate) fn decode_register_access(
        reader: &mut Reader<'_>,
        requires_service: bool,
    ) -> Result<OperationKind, CodecError> {
        let service = requires_service
            .then(|| reader.id("ServiceId"))
            .transpose()?;
        let operation = RegisterOperation::from_catalog_tag(reader.u16()?).ok_or(
            CodecError::MalformedStructuralFoundation("unknown register access catalog identity"),
        )?;
        if service.is_some() != operation.requires_service_authority() {
            return crate::codec_error::malformed(
                "register access wire tag disagrees with catalog service authority",
            );
        }
        let event = match operation {
            RegisterOperation::GlobalDescriptorTable(_)
            | RegisterOperation::TaskRegisterActivate
            | RegisterOperation::TaskRegisterRead
            | RegisterOperation::SegmentReload(_)
            | RegisterOperation::SegmentRead(_) => {
                return crate::codec_error::malformed(
                    "descriptor-table operations require a checked structural operand",
                );
            }
            RegisterOperation::MemoryFence(kind) => RegisterAccessEvent::MemoryFence { kind },
            RegisterOperation::ProcessorHalt => RegisterAccessEvent::ProcessorHalt,
            RegisterOperation::InterruptControl(kind) => {
                RegisterAccessEvent::InterruptControl { kind }
            }
            RegisterOperation::SystemRead(register) => RegisterAccessEvent::Read { register },
            RegisterOperation::SystemWrite(register) => RegisterAccessEvent::Write {
                register,
                value: reader.id("ValueId")?,
            },
            RegisterOperation::PortRead => RegisterAccessEvent::PortRead {
                port: reader.id("ValueId")?,
            },
            RegisterOperation::PortWrite => RegisterAccessEvent::PortWrite {
                port: reader.id("ValueId")?,
                value: reader.id("ValueId")?,
            },
            RegisterOperation::ModelSpecificRead => RegisterAccessEvent::ModelSpecificRead {
                index: reader.id("ValueId")?,
            },
            RegisterOperation::ModelSpecificWrite => RegisterAccessEvent::ModelSpecificWrite {
                index: reader.id("ValueId")?,
                value: reader.id("ValueId")?,
            },
        };
        Ok(OperationKind::RegisterAccess { service, event })
    }
}

pub(crate) mod scalar_operations {
    use super::super::wire_fields::{Id, wire_enum};
    use terminal_psi::{FloatResultPolicy, TrappingIntegerOperation};

    wire_enum! {
        TrappingInteger: TrappingIntegerOperation, "TrappingIntegerOperation" {
            1 => Add { left: Id, right: Id },
            2 => Subtract { left: Id, right: Id },
            3 => Multiply { left: Id, right: Id },
            4 => Divide { left: Id, right: Id },
            5 => Remainder { left: Id, right: Id },
            6 => ShiftLeft { value: Id, count: Id },
            7 => ShiftRight { value: Id, count: Id },
            8 => Convert { operand: Id },
        }
        FloatPolicy: FloatResultPolicy, "FloatResultPolicy" {
            1 => Trapping { raw_result: Id },
            4 => Saturating { raw_result: Id, eligible: Id },
        }
    }
}

pub(crate) mod storage_operations {
    use super::super::CodecError;
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{
        Argument, Field, Id, Optional, OptionalField, Path, Scalar, wire_record,
    };
    use crate::sections::semantic_module::structural_place_wire::{
        decode_structural_arguments, decode_structural_path, encode_structural_arguments,
        encode_structural_path,
    };
    use semantic_vocabulary::{ObligationId, PlaceId, StructuralFieldId, ValueId};
    use terminal_psi::{
        OperationKind, ReferenceStorageView, StructuralAccess, StructuralArgument,
        StructuralPathSegment, StructuralStoreDisposition,
    };

    wire_record! {
        ReferenceStorage: ReferenceStorageView { byte_offset: Id, stored_scalar: Optional<Scalar> }
    }

    pub(crate) fn encode_establish_reference(
        writer: &mut Writer,
        source: StructuralArgument,
        storage: Option<ReferenceStorageView>,
    ) -> Result<(), CodecError> {
        writer.u8(if storage.is_some() { 111 } else { 69 });
        Argument::write(writer, &source)?;
        if let Some(storage) = storage {
            ReferenceStorage::write(writer, &storage)?;
        }
        Ok(())
    }

    pub(crate) fn decode_establish_reference(
        reader: &mut Reader<'_>,
        has_storage_view: bool,
    ) -> Result<OperationKind, CodecError> {
        Ok(OperationKind::EstablishReference {
            source: Argument::read(reader)?,
            storage: has_storage_view
                .then(|| ReferenceStorage::read(reader))
                .transpose()?,
        })
    }

    pub(crate) fn encode_optional_path_op(
        writer: &mut Writer,
        unprojected_tag: u8,
        projected_tag: u8,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), CodecError> {
        writer.u8(if path.is_empty() {
            unprojected_tag
        } else {
            projected_tag
        });
        writer.id(place);
        if !path.is_empty() {
            encode_structural_path(writer, "operation path", path)?;
        }
        Ok(())
    }

    pub(crate) fn decode_nonempty_path(
        reader: &mut Reader<'_>,
        empty_reason: &'static str,
    ) -> Result<Vec<StructuralPathSegment>, CodecError> {
        let path = decode_structural_path(reader)?;
        if path.is_empty() {
            return crate::codec_error::malformed(empty_reason);
        }
        Ok(path)
    }

    pub(crate) fn encode_element_view_read(
        writer: &mut Writer,
        source: StructuralArgument,
        index: ValueId,
        length: ValueId,
        obligation: ObligationId,
        path: Vec<StructuralPathSegment>,
    ) -> Result<(), CodecError> {
        if source.access != StructuralAccess::SharedBorrow {
            return crate::codec_error::malformed(
                "element-view read requires shared observation access",
            );
        }
        writer.u8(if path.is_empty() { 81 } else { 92 });
        // Nonzero place IDs keep the existing whole-descriptor encodings.
        if source.path.is_empty() {
            writer.id(source.place);
        } else {
            writer.u64(0);
            encode_structural_arguments(writer, &[source])?;
        }
        writer.id(index);
        writer.id(length);
        writer.id(obligation);
        if !path.is_empty() {
            encode_structural_path(writer, "element view read path", &path)?;
        }
        Ok(())
    }

    pub(crate) fn decode_element_view_read(
        reader: &mut Reader<'_>,
        projected_element: bool,
    ) -> Result<OperationKind, CodecError> {
        let source = if let Some(place) = PlaceId::new(reader.u64()?) {
            StructuralArgument::whole(place, StructuralAccess::SharedBorrow)
        } else {
            let [source]: [StructuralArgument; 1] = decode_structural_arguments(reader)?
                .try_into()
                .map_err(|_| {
                    CodecError::MalformedStructuralFoundation(
                        "element-view read needs one descriptor",
                    )
                })?;
            if source.path.is_empty() || source.access != StructuralAccess::SharedBorrow {
                return crate::codec_error::malformed(
                    "noncanonical projected element-view descriptor",
                );
            }
            source
        };
        let index = reader.id("ValueId")?;
        let length = reader.id("ValueId")?;
        let obligation = reader.id("ObligationId")?;
        let path = if projected_element {
            decode_nonempty_path(reader, "empty projected element view read path")?
        } else {
            Vec::new()
        };
        Ok(OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        })
    }

    pub(crate) fn encode_write_only_primitive_store(
        writer: &mut Writer,
        destination: PlaceId,
        value: ValueId,
        path: Vec<StructuralPathSegment>,
        range_obligation: Option<ObligationId>,
    ) -> Result<(), CodecError> {
        if let Some(obligation) = range_obligation {
            writer.u8(116);
            writer.id(destination);
            encode_structural_path(writer, "restricted primitive destination path", &path)?;
            writer.id(value);
            writer.id(obligation);
            return Ok(());
        }
        encode_optional_path_op(writer, 43, 90, destination, &path)?;
        writer.id(value);
        Ok(())
    }

    pub(crate) fn encode_store_structural_field(
        writer: &mut Writer,
        destination: PlaceId,
        disposition: StructuralStoreDisposition,
        path: Vec<StructuralPathSegment>,
        field: Option<StructuralFieldId>,
        value: StructuralArgument,
    ) -> Result<(), CodecError> {
        writer.u8(match disposition {
            StructuralStoreDisposition::Vacant => 78,
            StructuralStoreDisposition::ReplaceDiscardable => 118,
        });
        writer.id(destination);
        Path::<false>::write(writer, &path)?;
        OptionalField::write(writer, &field)?;
        Argument::write(writer, &value)
    }

    pub(crate) fn decode_store_structural_field(
        reader: &mut Reader<'_>,
        disposition: StructuralStoreDisposition,
    ) -> Result<OperationKind, CodecError> {
        Ok(OperationKind::StoreStructuralField {
            disposition,
            destination: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
            field: OptionalField::read(reader)?,
            value: Argument::read(reader)?,
        })
    }
}
pub(crate) mod terminator_wire {

    use super::super::CodecError;
    use super::super::contract_wire::{CrashPredicate, Successor};
    use super::super::wire::{Reader, Writer};
    use super::super::wire_fields::{
        Arguments, ClaimBinding, ClaimTransfers, CleanupAction, CrashRoutes, Field, Id, Ids, List,
        Optional, Path, ProofTerms, ScalarTerms, U32, wire_enum, wire_record,
    };
    use terminal_psi::{ExternalCompletionKind, StructuralCaseSuccessorEdge, Terminator};

    wire_record! {
        CaseSuccessor: StructuralCaseSuccessorEdge {
            edge: Id,
            target: Id,
            case: Id,
            payload_fields: Ids,
            structural_arguments: Arguments,
            cleanup_actions: List<CleanupAction>,
        }
    }

    wire_enum! {
        TerminatorRow: Terminator, "Terminator" {
            20 => EliminateEmpty { edge: Id, source: Id, path: Path },
            19 => InvokeNoNormal {
                edge: Id,
                static_reach_binding: Optional<U32>,
                required_contract: Optional<Id>,
                callee: Id,
                arguments: Ids,
                erased_arguments: ScalarTerms,
                erased_proof_arguments: ProofTerms,
                structural_arguments: Arguments,
                claim_transfers: ClaimTransfers,
                requirement_obligations: Ids,
                crash_continuations: CrashRoutes,
            },
            17 => ExternalCompletion {
                edge: Id,
                boundary: Id,
                kind: CompletionKind,
                arguments: Ids,
                structural_arguments: Arguments,
                requirement_obligations: Ids,
                frontier_lower_bound: Ids,
            },
            14 => Jump {
                edge: Id,
                target: Id,
                arguments: Ids,
                erased_arguments: ScalarTerms,
                erased_proof_arguments: ProofTerms,
                structural_arguments: Arguments,
                cleanup_actions: List<CleanupAction>,
            },
            2 => Return { edge: Id, value: Id, cleanup_actions: List<CleanupAction> },
            13 => ReturnUnit { edge: Id, cleanup_actions: List<CleanupAction> },
            18 => ReturnStructural {
                edge: Id,
                source: Id,
                returned_claims: List<ClaimBinding>,
                cleanup_actions: List<CleanupAction>,
            },
            15 => Conditional { condition: Id, when_true: Successor, when_false: Successor },
            16 => StructuralCase { source: Id, path: Path, cases: List<CaseSuccessor> },
            4 => Crash {
                edge: Id,
                cause: super::super::contract_wire::Cause,
                site_guard: List<CrashPredicate>,
                frontier_lower_bound: Ids,
            },
        }
        CompletionKind: ExternalCompletionKind, "ExternalCompletionKind" { 1 => ProcessExit }
    }

    pub(crate) fn encode_terminator(
        writer: &mut Writer,
        terminator: &Terminator,
    ) -> Result<(), CodecError> {
        TerminatorRow::write(writer, terminator)
    }

    pub(crate) fn decode_terminator(reader: &mut Reader<'_>) -> Result<Terminator, CodecError> {
        TerminatorRow::read(reader)
    }
}

#[cfg(test)]
mod element_view_tests {
    use super::{Reader, Writer, decode_operation_kind, encode_operation_kind};
    use semantic_vocabulary::{ObligationId, PlaceId, ValueId};
    use terminal_psi::{
        OperationKind, StructuralAccess, StructuralArgument, StructuralPathSegment,
    };

    #[test]
    fn both_whole_descriptor_read_tags_keep_the_legacy_bytes() {
        use super::super::structural_place_wire::encode_structural_path;
        let source =
            StructuralArgument::whole(PlaceId::new(1).unwrap(), StructuralAccess::SharedBorrow);
        for (tag, path) in [
            (81, vec![]),
            (92, vec![StructuralPathSegment::FixedIndex(1)]),
        ] {
            let read = OperationKind::ElementViewRead {
                source: source.clone(),
                index: ValueId::new(2).unwrap(),
                length: ValueId::new(3).unwrap(),
                obligation: ObligationId::new(4).unwrap(),
                path: path.clone(),
            };
            let mut legacy = Writer::default();
            legacy.u8(tag);
            for id in 1..=4 {
                legacy.u64(id);
            }
            if !path.is_empty() {
                encode_structural_path(&mut legacy, "element view read path", &path).unwrap();
            }
            let legacy = legacy.finish();
            assert_eq!(
                decode_operation_kind(&mut Reader::new(&legacy)).unwrap(),
                read
            );
            let mut encoded = Writer::default();
            encode_operation_kind(&mut encoded, read).unwrap();
            assert_eq!(encoded.finish(), legacy);
        }
    }
}

#[cfg(test)]
mod owned_value_tests {
    use super::{Reader, Writer, decode_operation, encode_operation};
    use semantic_vocabulary::{
        ClaimId, OperationId, PlaceId, StructuralDomainId, StructuralTypeId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
        StructuralMultiplicity, StructuralOperationResult, StructuralPathQualification,
        StructuralPathSegment, StructuralResultClaimBinding, TerminalIdentityIndex,
    };

    #[test]
    fn packed_integer_wire_preserves_both_operand_paths_and_refuses_unknown_arithmetic() {
        let kind = OperationKind::PackedIntegerBinary {
            operation: semantic_vocabulary::PackedIntegerBinaryOperation::WrappingAddI16x8,
            left: StructuralArgument::whole(
                PlaceId::new(1).unwrap(),
                StructuralAccess::SharedBorrow,
            ),
            right: StructuralArgument {
                place: PlaceId::new(2).unwrap(),
                access: StructuralAccess::SharedBorrow,
                path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
            },
        };
        let mut writer = Writer::default();
        super::encode_operation_kind(&mut writer, kind.clone()).unwrap();
        let mut bytes = writer.finish();
        assert_eq!(&bytes[..2], &[142, 0]);
        let mut reader = Reader::new(&bytes);
        assert_eq!(super::decode_operation_kind(&mut reader).unwrap(), kind);
        assert_eq!(reader.remaining(), 0);
        bytes[1] = 255;
        assert!(matches!(
            super::decode_operation_kind(&mut Reader::new(&bytes)),
            Err(super::CodecError::InvalidTag(
                "PackedIntegerBinaryOperation",
                255
            ))
        ));
    }

    #[test]
    fn packed_integer_immediates_round_trip_and_invalid_indices_are_refused() {
        use semantic_vocabulary::{
            PackedIntegerBinaryOperation::*, PackedIntegerUnaryOperation::*,
        };
        let operand =
            StructuralArgument::whole(PlaceId::new(1).unwrap(), StructuralAccess::SharedBorrow);
        let binary = |operation| OperationKind::PackedIntegerBinary {
            operation,
            left: operand.clone(),
            right: operand.clone(),
        };
        let unary = |operation| OperationKind::PackedIntegerUnary {
            operation,
            source: operand.clone(),
        };
        let bytes = |kind| {
            let mut writer = Writer::default();
            super::encode_operation_kind(&mut writer, kind).unwrap();
            writer.finish()
        };
        for kind in [
            binary(WrappingSubI16x8),
            binary(ShuffleI16x8 {
                lanes: [1, 15, 0, 8, 7, 7, 14, 2],
            }),
            unary(ShiftRightArithmeticI16x8 { count: 15 }),
            unary(WidenU8x8ToI16x8),
            unary(NarrowI16x8ToU8x8Saturating),
            unary(LoadI16x4ZeroExtend),
            unary(WidenU8x4ToI16x8),
            unary(NarrowI16x8ToU8x4Saturating),
            unary(AbsI16x8),
            binary(WrappingAddU8x16),
            binary(MinI16x8),
            binary(CmpGtI16x8),
            binary(BitAndI16x8),
            binary(BitAndNotI16x8),
        ] {
            let encoded = bytes(kind.clone());
            assert_eq!(
                super::decode_operation_kind(&mut Reader::new(&encoded)).unwrap(),
                kind
            );
        }
        for kind in [
            unary(ShiftRightArithmeticI16x8 { count: 16 }),
            binary(ShuffleI16x8 { lanes: [16; 8] }),
        ] {
            assert!(super::decode_operation_kind(&mut Reader::new(&bytes(kind))).is_err());
        }
    }

    #[test]
    fn empty_observation_wire_retains_exact_source_and_payload_namespace() {
        use semantic_vocabulary::{EdgeId, StructuralCaseId};
        use terminal_psi::Terminator;
        let terminal = Terminator::EliminateEmpty {
            edge: EdgeId::new(1).unwrap(),
            source: PlaceId::new(2).unwrap(),
            path: vec![
                StructuralPathSegment::Field(TerminalIdentityIndex(4)),
                StructuralPathSegment::Case(StructuralCaseId::new(3).unwrap()),
            ],
        };
        let mut writer = Writer::default();
        super::terminator_wire::encode_terminator(&mut writer, &terminal).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[0], 20);
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            super::terminator_wire::decode_terminator(&mut reader).unwrap(),
            terminal
        );
        assert_eq!(reader.remaining(), 0);
        let mut changed = terminal;
        let Terminator::EliminateEmpty { source, .. } = &mut changed else {
            unreachable!()
        };
        *source = PlaceId::new(5).unwrap();
        let mut writer = Writer::default();
        super::terminator_wire::encode_terminator(&mut writer, &changed).unwrap();
        assert_ne!(
            bytes,
            writer.finish(),
            "source identity participates in the artifact's meaning"
        );
    }

    #[test]
    fn placed_reference_wire_preserves_owner_roles_and_backing_kind() {
        let source = StructuralArgument {
            place: PlaceId::new(2).unwrap(),
            access: StructuralAccess::MutableBorrow,
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        };
        let resident = terminal_psi::ResidentReferenceSource {
            base: semantic_vocabulary::StructuralFieldId::new(7).unwrap(),
            length: semantic_vocabulary::StructuralFieldId::new(8).unwrap(),
            granted: StructuralDomainId::new(9).unwrap(),
            resident: StructuralDomainId::new(10).unwrap(),
        };
        let mut encodings = Vec::new();
        for roles in [
            None,
            Some(resident),
            Some(terminal_psi::ResidentReferenceSource {
                base: resident.length,
                length: resident.base,
                ..resident
            }),
        ] {
            let kind = OperationKind::EstablishAccessReference {
                source: source.clone(),
                resident: roles,
            };
            let mut writer = Writer::default();
            super::encode_operation_kind(&mut writer, kind.clone()).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[0], 120);
            let mut reader = Reader::new(&bytes);
            assert_eq!(super::decode_operation_kind(&mut reader).unwrap(), kind);
            assert_eq!(reader.remaining(), 0);
            encodings.push(bytes);
        }
        assert_ne!(encodings[0], encodings[1]);
        assert_ne!(encodings[1], encodings[2]);
    }

    #[test]
    fn consuming_decomposition_has_its_own_tag_and_exact_subject() {
        let mut encodings = Vec::new();
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        ] {
            let subject = StructuralArgument {
                place: PlaceId::new(2).unwrap(),
                path,
                access: StructuralAccess::Owned,
            };
            let kind = OperationKind::DecomposeStructuralValue {
                subject: subject.clone(),
            };
            let mut writer = Writer::default();
            super::encode_operation_kind(&mut writer, kind.clone()).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[0], 140);
            let mut reader = Reader::new(&bytes);
            assert_eq!(super::decode_operation_kind(&mut reader).unwrap(), kind);
            assert_eq!(reader.remaining(), 0);
            let mut moved = Writer::default();
            super::encode_operation_kind(
                &mut moved,
                OperationKind::MoveStructuralValue { source: subject },
            )
            .unwrap();
            assert_ne!(bytes, moved.finish());
            assert!(!encodings.contains(&bytes));
            encodings.push(bytes);
        }
    }

    #[test]
    fn owned_value_wire_preserves_projected_source_and_claim_identity() {
        let path = vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))];
        let operation = Operation {
            id: OperationId::new(9).unwrap(),
            static_reach_binding: None,
            required_contract: None,
            suspension_crossing: None,
            kind: OperationKind::MoveStructuralValue {
                source: StructuralArgument {
                    place: PlaceId::new(2).unwrap(),
                    path,
                    access: StructuralAccess::Owned,
                },
            },
            result: OperationResult::Structural(StructuralOperationResult {
                place: PlaceId::new(3).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: vec![StructuralDomainId::new(5).unwrap()],
                projected_qualifications: vec![StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(6))],
                    domain: StructuralDomainId::new(7).unwrap(),
                }],
                additional_input_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: vec![StructuralResultClaimBinding {
                    claim: ClaimId::new(8).unwrap(),
                    path: Vec::new(),
                }],
            }),
        };
        let mut writer = Writer::default();
        encode_operation(&mut writer, &operation).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_operation(&mut reader).unwrap(), operation);
        assert_eq!(reader.remaining(), 0);

        let mut copy = operation.clone();
        copy.kind = OperationKind::StructuralLeafCopy {
            source: PlaceId::new(2).unwrap(),
            path: vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
        };
        let mut writer = Writer::default();
        encode_operation(&mut writer, &copy).unwrap();
        assert_ne!(
            bytes,
            writer.finish(),
            "copy and move must have distinct wire meanings"
        );
    }
}
#[cfg(test)]
mod unknown_tag_tests {

    use super::super::entry_tests::unit_machine;
    use super::{Writer, encode_operation};
    use crate::{CodecError, decode_module, encode_module};
    use semantic_vocabulary::{
        MachineId, OperationId, ScalarQualificationSetId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Operation, OperationKind, OperationResult, TerminalModule, ValueDeclaration,
    };

    fn module_with_one_operation() -> (TerminalModule, Vec<u8>, usize) {
        let machine_id = MachineId::new(1).unwrap();
        let operation = Operation {
            id: OperationId::new(1).unwrap(),
            static_reach_binding: None,
            required_contract: None,
            suspension_crossing: None,
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
                qualifications: ScalarQualificationSetId::ZERO,
            }),
            kind: OperationKind::BooleanConstant { value: true },
        };
        let mut machine = unit_machine(machine_id);
        machine.blocks[0].operations.push(operation.clone());
        let mut module = TerminalModule::for_entry(machine_id);
        module.machines.push(machine);
        let bytes = encode_module(&module).unwrap();

        let mut writer = Writer::default();
        encode_operation(&mut writer, &operation).unwrap();
        let row = writer.finish();
        let start = bytes
            .windows(row.len())
            .position(|window| window == row.as_slice())
            .expect("the encoded module contains the operation row");
        let tag_offset = start + row.len() - 3;
        assert_eq!(bytes[tag_offset], 2);
        (module, bytes, tag_offset)
    }

    #[test]
    fn an_operation_tag_without_a_table_row_rejects_the_module() {
        let (module, bytes, tag_offset) = module_with_one_operation();
        assert_eq!(decode_module(&bytes).unwrap(), module);
        let listed = crate::spec_tag_tables::listed_tags("operation-tags");
        for tag in (0..=u8::MAX).filter(|tag| !listed.contains(tag)) {
            let mut changed = bytes.clone();
            changed[tag_offset] = tag;
            assert!(
                matches!(
                    decode_module(&changed),
                    Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
                ),
                "operation tag {tag} has no table row and must reject as an unknown operation tag"
            );
        }
    }

    #[test]
    fn unassigned_operation_tags_71_and_72_reject() {
        let (_, bytes, tag_offset) = module_with_one_operation();
        for tag in [71, 72] {
            let mut changed = bytes.clone();
            changed[tag_offset] = tag;
            assert!(matches!(
                decode_module(&changed),
                Err(CodecError::InvalidTag("OperationKind", rejected)) if rejected == tag
            ));
        }
    }
}

use terminal_psi::{Block, Operation, OperationKind, OperationResult};

use super::CodecError;
use super::machine_wire::{decode_declaration, encode_declaration};
use super::proof_term_wire::{decode_proof_formals, encode_proof_formals};
use super::structural_result_wire::{decode_operation_result, encode_operation_result};
use super::structural_signature_wire::StructuralParameter;
use super::wire::{Reader, Writer};
use super::wire_fields::{
    self, Field, Id, List, Optional, U32, delegated_fields, wire_enum, wire_record,
};

struct ValueDeclarationField;
impl Field<terminal_psi::ValueDeclaration> for ValueDeclarationField {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::ValueDeclaration,
    ) -> Result<(), CodecError> {
        encode_declaration(writer, *value)
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::ValueDeclaration, CodecError> {
        decode_declaration(reader)
    }
}

delegated_fields! {
    StructuralResultField: terminal_psi::StructuralOperationResult
        => encode_operation_result, decode_operation_result;
    ProofFormalsField: Vec<terminal_psi::ErasedProofFormal>
        => encode_proof_formals, decode_proof_formals;
    TerminatorField: terminal_psi::Terminator
        => terminator_wire::encode_terminator, terminator_wire::decode_terminator;
}

struct AssemblyExit;
impl Field<terminal_psi::AssemblyExitConstraint> for AssemblyExit {
    fn write(
        writer: &mut Writer,
        value: &terminal_psi::AssemblyExitConstraint,
    ) -> Result<(), CodecError> {
        match value {
            terminal_psi::AssemblyExitConstraint::Adjacent(target) => {
                writer.u8(1);
                Id::write(writer, target)?;
            }
            terminal_psi::AssemblyExitConstraint::ScalarReturn(view) => {
                writer.u8(2);
                writer.u8(view.slot());
            }
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<terminal_psi::AssemblyExitConstraint, CodecError> {
        match reader.u8()? {
            1 => Ok(terminal_psi::AssemblyExitConstraint::Adjacent(Id::read(
                reader,
            )?)),
            2 => terminal_psi::AsmRegisterView::from_scratch_slot(usize::from(reader.u8()?))
                .map(terminal_psi::AssemblyExitConstraint::ScalarReturn)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "unknown assembly return register view",
                )),
            _ => Err(CodecError::MalformedStructuralFoundation(
                "unknown assembly exit constraint",
            )),
        }
    }
}

struct OperationKindField;
impl Field<OperationKind> for OperationKindField {
    fn write(writer: &mut Writer, kind: &OperationKind) -> Result<(), CodecError> {
        encode_operation_kind(writer, kind.clone())
    }
    fn read(reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
        decode_operation_kind(reader)
    }
}

wire_enum! {
    OperationResultField: OperationResult, "OperationResult" {
        0 => Unit,
        1 => Scalar(value: ValueDeclarationField),
        2 => Structural(value: StructuralResultField),
    }
}

wire_record! {
    OperationRow: Operation {
        id: Id,
        static_reach_binding: Optional<U32>,
        required_contract: Optional<Id>,
        result: OperationResultField,
        kind: OperationKindField,
        suspension_crossing: Optional<Id>,
    }
    BlockRow: Block {
        id: Id,
        parameters: List<ValueDeclarationField>,
        erased_scalar_formals: List<ValueDeclarationField>,
        erased_proof_formals: ProofFormalsField,
        structural_parameters: List<StructuralParameter>,
        operations: List<OperationRow>,
        terminator: TerminatorField,
        assembly_exit: Optional<AssemblyExit>,
    }
}

pub(crate) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    BlockRow::write(writer, block)
}

pub(crate) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    BlockRow::read(reader)
}

#[cfg(test)]
fn encode_operation(writer: &mut Writer, operation: &Operation) -> Result<(), CodecError> {
    OperationRow::write(writer, operation)
}

#[cfg(test)]
pub(crate) fn decode_operation(reader: &mut Reader<'_>) -> Result<Operation, CodecError> {
    OperationRow::read(reader)
}

macro_rules! operation_wire {
    (
        rows { $($tag:literal => $variant:ident { $($field:ident: $kind:ty),* $(,)? }),* $(,)? }
        paired {
            $($absent:literal | $present:literal => $paired:ident {
                $($paired_field:ident: $paired_kind:ty),* ; $optional:ident
            }),* $(,)?
        }
        encode($writer:ident) { $($encode_pattern:pat => $encode_body:expr),* $(,)? }
        decode($reader:ident) { $($decode_tag:pat => $decode_body:expr),* $(,)? }
    ) => {
        fn encode_operation_kind(
            $writer: &mut Writer,
            kind: OperationKind,
        ) -> Result<(), CodecError> {
            use wire_fields::*;
            match kind {
                $(OperationKind::$variant { $($field),* } => {
                    $writer.u8($tag);
                    $(<$kind as Field<_>>::write($writer, &$field)?;)*
                })*
                $(OperationKind::$paired { $($paired_field,)* $optional } => {
                    $writer.u8(if $optional.is_some() { $present } else { $absent });
                    $(<$paired_kind as Field<_>>::write($writer, &$paired_field)?;)*
                    if let Some(obligation) = $optional {
                        $writer.id(obligation);
                    }
                })*
                $($encode_pattern => {
                    $encode_body;
                })*
            }
            Ok(())
        }

        pub(crate) fn decode_operation_kind($reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
            use wire_fields::*;
            Ok(match $reader.u8()? {
                $($tag => OperationKind::$variant {
                    $($field: <$kind as Field<_>>::read($reader)?),*
                },)*
                $($absent => OperationKind::$paired {
                    $($paired_field: <$paired_kind as Field<_>>::read($reader)?,)*
                    $optional: None,
                },
                $present => OperationKind::$paired {
                    $($paired_field: <$paired_kind as Field<_>>::read($reader)?,)*
                    $optional: Some($reader.id("ObligationId")?),
                },)*
                $($decode_tag => $decode_body,)*
                tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
            })
        }
    };
}

operation_wire! {
    rows {
        1 => IntegerConstant { value: Integer },
        2 => BooleanConstant { value: Flag },
        3 => WrappingIntegerAdd { left: Id, right: Id },
        4 => SaturatingIntegerAdd { left: Id, right: Id },
        5 => WrappingIntegerSubtract { left: Id, right: Id },
        6 => SaturatingIntegerSubtract { left: Id, right: Id },
        7 => WrappingIntegerMultiply { left: Id, right: Id },
        8 => SaturatingIntegerMultiply { left: Id, right: Id },
        9 => BooleanNot { operand: Id },
        10 => BooleanEqual { left: Id, right: Id },
        11 => IntegerEqual { left: Id, right: Id },
        12 => IntegerLessThan { left: Id, right: Id },
        13 => IntegerLessOrEqual { left: Id, right: Id },
        14 => IntegerBitwiseAnd { left: Id, right: Id },
        15 => IntegerBitwiseOr { left: Id, right: Id },
        16 => IntegerBitwiseXor { left: Id, right: Id },
        17 => WrappingIntegerShiftLeft { value: Id, count: Id },
        18 => WrappingIntegerShiftRight { value: Id, count: Id },
        19 => IntegerBitwiseNot { operand: Id },
        20 => IntegerWiden { operand: Id },
        21 => IntegerExactCast { operand: Id, obligation: Id },
        22 => ExactIntegerShiftRight { value: Id, count: Id, obligation: Id },
        23 => ExactIntegerShiftLeft { value: Id, count: Id, obligation: Id },
        24 => ExactIntegerAdd { left: Id, right: Id, obligation: Id },
        25 => ExactIntegerSubtract { left: Id, right: Id, obligation: Id },
        26 => ExactIntegerMultiply { left: Id, right: Id, obligation: Id },
        27 => ExactIntegerDivide { left: Id, right: Id, obligation: Id },
        28 => ExactIntegerRemainder { left: Id, right: Id, obligation: Id },
        29 => WrappingIntegerDivide { left: Id, right: Id, obligation: Id },
        30 => WrappingIntegerRemainder { left: Id, right: Id, obligation: Id },
        31 => SaturatingIntegerDivide { left: Id, right: Id, obligation: Id },
        32 => SaturatingIntegerRemainder { left: Id, right: Id, obligation: Id },
        33 => Call {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        34 => CallUnit {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        35 => BoundaryCall {
            boundary: Id,
            arguments: Ids,
            structural_arguments: Arguments,
            completion_receipts: CompletionReceipts,
            requirement_obligations: Ids,
        },
        36 => PortWrite { service: Id, port: U16, value: U8 },
        37 => EstablishTrivialAffineLocal { destination: Id },
        38 => BooleanStructuralField { source: Id, path: FieldPath, field: Id },
        39 => CallStructuralScalar {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        40 => EstablishByteSequenceLiteral { destination: Id, bytes: Bytes, qualifications: Ids },
        41 => CallStructural {
            callee: Id,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        42 => EstablishScalarCase { result_case: Id, fields: ScalarCaseFields },
        44 => IeeeFloatConstant { value: Float },
        45 => NearestIeeeFloatFusedMultiplyAdd { left: Id, right: Id, addend: Id },
        47 => ScalarStructuralField { source: Id, path: FieldPath, field: Id },
        50 => CallStructuralWithScalarArguments {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        56 => ByteSequenceRead { source: Id, index: Id, length: Id, obligation: Id },
        58 => StructuralByteSequenceFieldStore {
            destination: Id, path: Path, field: Id, source: Id, length: Id, obligation: Id,
        },
        59 => StructuralByteSequenceFieldLength { source: Id, path: Path, field: Id },
        60 => StructuralByteSequenceFieldByteStore {
            destination: Id, path: Path, field: Id, index: Id, value: Id, length: Id, obligation: Id,
        },
        63 => ByteSequenceWrite { destination: Id, index: Id, value: Id, length: Id, obligation: Id },
        64 => EstablishScalarArray { elements: Ids },
        65 => IeeeFloatCompare { comparison: FloatComparison, left: Id, right: Id },
        66 => StructuralCaseMembership { source: Id, path: Path, case: Id },
        68 => EstablishRecord { fields: RecordFields },
        77 => MoveStructuralField { source: Id, path: Path, field: OptionalField },
        79 => EstablishElementView { destination: Id, source: Argument, element: Id, region: Region },
        83 => EstablishStructuralCase { result_case: Id, fields: RecordFields },
        84 => StructuralLeafCopy { source: Id, path: Path },
        86 => StructuralByteSequenceFieldRead {
            source: Id, path: Path, field: Id, index: Id, length: Id, obligation: Id,
        },
        91 => TrappingInteger { operation: scalar_operations::TrappingInteger },
        94 => EstablishZeroedStructural {},
        95 => EstablishByteSequenceConcatenation {
            left: ByteOperand, right: ByteOperand, capacity_obligation: Id,
        },
        96 => ByteSequenceStore { destination: Id, source: Id, length: Id, obligation: Id },
        97 => ElementViewWrite {
            destination: Argument, path: Path, index: Id, value: Id, length: Id, obligation: Id,
        },
        98 => EstablishMutableElementView { destination: Id, source: Argument, element: Id, region: Region },
        99 => FloatResultPolicy { policy: scalar_operations::FloatPolicy },
        103 => EstablishStructuralLocal {},
        104 => ByteSequencePredicate { operand: ByteOperand, predicate: Predicate },
        105 => ByteSequenceEqual { left: ByteOperand, right: ByteOperand },
        107 => EstablishStructuralArray { elements: Arguments },
        109 => SelectImplementation { table: U32 },
        110 => CallDynamic {
            selector: Id,
            interface: U32,
            slot: U32,
            arguments: Ids,
            family_actuals: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        112 => ElementViewLength { source: Argument },
        113 => ByteSequenceSubslice { source: Argument, start: Id, end: Id, length: Id, obligation: Id },
        114 => ElementViewSubslice { source: Argument, start: Id, end: Id, length: Id, obligation: Id },
        115 => ByteSequenceLength { source: Argument },
        117 => EstablishPrimitiveLocal { value: Id, range_obligation: OptionalId },
        119 => AtomicFence { ordering: atomic_operations::Fence },
        120 => EstablishAccessReference { source: Argument, resident: Resident },
        122 => EstablishNativeCodeLayout { machine: Id },
        123 => StorageAddress { source: Argument },
        124 => MoveStructuralValue { source: Argument },
        127 => EstablishActivationClaim {
            bound: U64, committed: Id, backing_type: Id, established_case: Id, rejected_case: Id,
        },
        128 => ReleaseActivationClaim { source: Argument, claim: Id },
        129 => SplitActivationClaim {
            source: Argument, claim: Id, bound_at: U64, committed_at: Id, covered_case: Id, rejected_case: Id,
        },
        130 => MergeActivationClaim {
            lower: Argument, upper: Argument, lower_claim: Id, upper_claim: Id, covered_case: Id, rejected_case: Id,
        },
        132 => ReadScalarApplication { application: Id },
        133 => GlobalDescriptorTable { service: Id, operation: GlobalDescriptorTable, source: Argument },
        134 => ActivateTaskRegister {
            service: Id, entries: U16, gdt: Argument, task_state: Argument, selector: Id, scratch: Argument,
        },
        135 => ReadTaskRegister { service: Id },
        136 => ReloadSegmentDescriptors {
            service: Id, entries: U16, kind: SegmentReloadKind, gdt: Argument, selector: Id, scratch: Argument,
        },
        137 => ReadSegmentSelector { service: Id, register: SegmentSelectorRegister },
        138 => SelectMachine { descriptor: Id },
        139 => CallMachine {
            callee: Id,
            contract: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        140 => DecomposeStructuralValue { subject: Argument },
        141 => StorageAlignment { source: Argument },
        142 => PackedIntegerBinary { operation: PackedIntegerBinary, left: Argument, right: Argument },
        143 => PackedIntegerUnary { operation: PackedIntegerUnary, source: Argument },
        144 => ElementViewFixedArray { source: Argument, length: Id, extent: U64, obligation: Id },
    }
    paired {
        100 | 106 => StructuralByteViewFieldStore {
            destination: Id, path: Path, field: OptionalField, source: Id; predicate_obligation
        },
        46 | 75 => StructuralScalarFieldStore {
            destination: Id, path: Path, field: Id, value: Id; range_obligation
        },
        101 | 102 => ScalarRepresentationView { operand: Id; landing_obligation },
    }
    encode(writer) {
        OperationKind::EstablishReference { source, storage } => {
            storage_operations::encode_establish_reference(writer, source, storage)?
        },
        OperationKind::ReleaseReference { source, path } => {
            storage_operations::encode_optional_path_op(writer, 70, 108, source, &path)?
        },
        OperationKind::PrimitiveScalarRead { source, path } => {
            storage_operations::encode_optional_path_op(writer, 62, 89, source, &path)?
        },
        OperationKind::TransformContent { source, parents, derivations } => {
            writer.u8(125);
            super::structural_place_wire::encode_structural_arguments(writer, &[source])?;
            super::structural_place_wire::encode_structural_arguments(writer, &parents)?;
            super::content_wire::encode_content_transformations(writer, &derivations)?;
        },
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            writer.u8(85);
            super::structural_field_wire::encode_canonical_structural_field(
                writer, source, &path, "case leaf copy path",
            )?;
        },
        OperationKind::WriteOnlyPrimitiveStore { destination, value, path, range_obligation } => {
            storage_operations::encode_write_only_primitive_store(
                writer, destination, value, path, range_obligation,
            )?
        },
        OperationKind::StoreStructuralField { disposition, destination, path, field, value } => {
            storage_operations::encode_store_structural_field(
                writer, destination, disposition, path, field, value,
            )?
        },
        OperationKind::ElementViewRead { source, index, length, obligation, path } => {
            storage_operations::encode_element_view_read(
                writer, source, index, length, obligation, path,
            )?
        },
        OperationKind::RegisterAccess { service, event } => {
            call_operations::encode_register_access(writer, service, event)?
        },
        OperationKind::AtomicAccess { place, path, event, resident_obligation } => {
            atomic_operations::encode_atomic_access(writer, place, &path, event, resident_obligation)?
        },
    }
    decode(reader) {
        69 => storage_operations::decode_establish_reference(reader, false)?,
        111 => storage_operations::decode_establish_reference(reader, true)?,
        70 => OperationKind::ReleaseReference { source: reader.id("PlaceId")?, path: Vec::new() },
        108 => OperationKind::ReleaseReference {
            source: reader.id("PlaceId")?,
            path: storage_operations::decode_nonempty_path(reader, "projected release requires a nonempty path")?,
        },
        62 => OperationKind::PrimitiveScalarRead { source: reader.id("PlaceId")?, path: Vec::new() },
        89 => OperationKind::PrimitiveScalarRead {
            source: reader.id("PlaceId")?,
            path: storage_operations::decode_nonempty_path(reader, "empty projected primitive path")?,
        },
        125 => {
            let [source]: [terminal_psi::StructuralArgument; 1] =
                super::structural_place_wire::decode_structural_arguments(reader)?
                    .try_into()
                    .map_err(|_| CodecError::MalformedStructuralFoundation(
                        "content transformation requires one payload",
                    ))?;
            OperationKind::TransformContent {
                source,
                parents: super::structural_place_wire::decode_structural_arguments(reader)?,
                derivations: super::content_wire::decode_content_transformations(reader)?,
            }
        },
        85 => {
            let (source, path) = super::structural_field_wire::decode_canonical_structural_field(reader)?;
            OperationKind::StructuralCaseLeafCopy { source, path }
        },
        43 => OperationKind::WriteOnlyPrimitiveStore {
            destination: reader.id("PlaceId")?,
            path: Vec::new(),
            value: reader.id("ValueId")?,
            range_obligation: None,
        },
        90 => OperationKind::WriteOnlyPrimitiveStore {
            destination: reader.id("PlaceId")?,
            path: storage_operations::decode_nonempty_path(reader, "empty projected primitive path")?,
            value: reader.id("ValueId")?,
            range_obligation: None,
        },
        116 => OperationKind::WriteOnlyPrimitiveStore {
            destination: reader.id("PlaceId")?,
            path: super::structural_place_wire::decode_structural_path(reader)?,
            value: reader.id("ValueId")?,
            range_obligation: Some(reader.id("ObligationId")?),
        },
        78 => storage_operations::decode_store_structural_field(reader, terminal_psi::StructuralStoreDisposition::Vacant)?,
        118 => storage_operations::decode_store_structural_field(reader, terminal_psi::StructuralStoreDisposition::ReplaceDiscardable)?,
        81 => storage_operations::decode_element_view_read(reader, false)?,
        92 => storage_operations::decode_element_view_read(reader, true)?,
        121 => call_operations::decode_register_access(reader, true)?,
        126 => call_operations::decode_register_access(reader, false)?,
        93 => atomic_operations::decode_atomic_access(reader, false)?,
        131 => atomic_operations::decode_atomic_access(reader, true)?,
    }
}
