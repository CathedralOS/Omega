use super::{
    BlockId, Cursor, EdgeId, SelectedBlockId, SelectedBlockOrigin, SelectedSuccessor,
    SelectedSuccessorRole, SelectedValueBinding, SelectedValueTransport, ValueBinding, ValueId,
    VirtualRegisterId,
};
use crate::FixedViewCopyDecodeError;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::block::decode_block;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::block::decode_successor;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::block::encode_block;
use crate::register_homes::recovery::fixed_view_copy::codec::selected::block::encode_successor;
use semantic_vocabulary::ScalarType;
fn successor() -> SelectedSuccessor {
    SelectedSuccessor {
        role: SelectedSuccessorRole::Semantic,
        structural_case: None,
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(1).unwrap(),
        block: SelectedBlockId(2),
        source_target: BlockId::new(3).unwrap(),
        bindings: vec![SelectedValueBinding {
            semantic: ValueBinding {
                parameter: ValueId::new(4).unwrap(),
                argument: ValueId::new(5).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(17),
                parameter: VirtualRegisterId(29),
            },
        }],
        fuel: vec![],
    }
}

#[test]
fn successor_scalar_reference_preserves_both_storage_interpretations() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedAddressBase, SelectedScalarReferenceSource,
        SelectedStructuralBinding, SelectedStructuralTransport,
    };
    use semantic_vocabulary::PlaceId;
    let mut original = successor();
    let destination = LocalStorageSlotId::StructuralBlockParameter {
        block: original.source_target,
        place: PlaceId::new(31).unwrap(),
    };
    original
        .structural_bindings
        .push(SelectedStructuralBinding {
            source_indices: vec![legalized_operations::LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: ValueId::new(14).unwrap(),
                    scalar_type: ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .unwrap(),
                    ),
                },
                stride: 16,
                extent: None,
                obligation: semantic_vocabulary::ObligationId::new(15).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [16; 32],
                ),
            }],
            semantic: abstract_operations::AbstractStructuralBinding {
                parameter: PlaceId::new(31).unwrap(),
                argument: terminal_psi::StructuralArgument {
                    place: PlaceId::new(9).unwrap(),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::MutableBorrow,
                },
            },
            transport: SelectedStructuralTransport::Unused,
        });
    for (base, source) in [
        (
            SelectedAddressBase::Register(VirtualRegisterId(41)),
            SelectedScalarReferenceSource::Canonical { codec: 257 },
        ),
        (
            SelectedAddressBase::Local(destination),
            SelectedScalarReferenceSource::Descriptor,
        ),
    ] {
        original.structural_bindings[0].transport = SelectedStructuralTransport::ScalarReference {
            base,
            byte_offset: 0,
            source,
            destination,
        };
        let mut bytes = Vec::new();
        encode_successor(&mut bytes, &original);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_successor(&mut Cursor::new(&bytes[..length])).is_err());
        }
    }
    original.structural_bindings[0].transport = SelectedStructuralTransport::ScalarReference {
        base: SelectedAddressBase::Register(VirtualRegisterId(41)),
        byte_offset: 0,
        source: SelectedScalarReferenceSource::Canonical { codec: 9 },
        destination,
    };
    let mut invalid = Vec::new();
    encode_successor(&mut invalid, &original);
    assert_eq!(
        decode_successor(&mut Cursor::new(&invalid)),
        Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
    );
}
#[test]
fn successor_register_transport_has_exact_canonical_bytes() {
    let original = successor();
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    let golden = "0001000000000000000200000003000000000000000100000000000000040000000000000005000000000000000001110000001d0000000000000000000000000000000000000000";
    let expected = (0..golden.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&golden[offset..offset + 2], 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(encoded, expected);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_successor(&mut cursor).unwrap(), original);
    assert_eq!(cursor.remaining(), 0);
    let mut unused = original;
    unused.bindings[0].transport = SelectedValueTransport::Unused;
    let mut unused_encoded = Vec::new();
    encode_successor(&mut unused_encoded, &unused);
    assert_eq!(unused_encoded.len(), encoded.len() - 8);
    assert_eq!(unused_encoded[46], 0);
    assert_eq!(
        decode_successor(&mut Cursor::new(&unused_encoded)).unwrap(),
        unused
    );
}

#[test]
fn crash_block_codec_retains_exact_cause_guards_frontier_and_fuel() {
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use selected_instructions::{
        SelectedBlock, SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
        SelectedInstructionProvenance, SelectedTerminator,
    };
    use semantic_vocabulary::{ClaimId, Proposition};
    use terminal_psi::{CrashCause, CrashPredicateTerm};

    for cause in [CrashCause::Trap, CrashCause::Abort] {
        let edge = EdgeId::new(7).unwrap();
        let block = SelectedBlock {
            id: SelectedBlockId(0),
            origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Crash {
                instruction: SelectedInstruction {
                    id: SelectedInstructionId(0),
                    kind: SelectedInstructionKind::Crash,
                    constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 780,
                    },
                    operands: Vec::new(),
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: SelectedInstructionProvenance {
                        edges: vec![edge],
                        fuel: vec![optimization_unit::FuelSettlement {
                            site: optimization_unit::PsiProvenance::Edge(edge),
                            units: 2,
                        }],
                        ..Default::default()
                    },
                },
                psi_edge: edge,
                cause,
                site_guard: vec![CrashPredicateTerm::new(Proposition::Truth)],
                frontier_lower_bound: vec![ClaimId::new(3).unwrap(), ClaimId::new(5).unwrap()],
            },
        };
        let mut encoded = Vec::new();
        encode_block(&mut encoded, &block);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_block(&mut cursor).unwrap(), block);
        assert_eq!(cursor.remaining(), 0);
        for mutation in 0..4 {
            let mut changed = block.clone();
            let SelectedTerminator::Crash {
                psi_edge,
                cause,
                site_guard,
                frontier_lower_bound,
                ..
            } = &mut changed.terminator
            else {
                unreachable!()
            };
            match mutation {
                0 => *psi_edge = EdgeId::new(9).unwrap(),
                1 => {
                    *cause = if *cause == CrashCause::Trap {
                        CrashCause::Abort
                    } else {
                        CrashCause::Trap
                    }
                }
                2 => site_guard.clear(),
                3 => frontier_lower_bound.reverse(),
                _ => unreachable!(),
            }
            let mut changed_bytes = Vec::new();
            encode_block(&mut changed_bytes, &changed);
            assert_ne!(changed_bytes, encoded);
            assert_eq!(
                decode_block(&mut Cursor::new(&changed_bytes)).unwrap(),
                changed
            );
        }
        let mut cursor = Cursor::new(&encoded);
        cursor.take(4 + 1 + 8 + 8 + 1).unwrap();
        super::decode_instruction(&mut cursor).unwrap();
        cursor.take(8).unwrap();
        let cause_offset = encoded.len() - cursor.remaining();
        let mut corrupt = encoded.clone();
        corrupt[cause_offset] = 255;
        assert_eq!(
            decode_block(&mut Cursor::new(&corrupt)),
            Err(FixedViewCopyDecodeError::UnknownCrashCause(255))
        );
        assert!(decode_block(&mut Cursor::new(&encoded[..encoded.len() - 1])).is_err());
    }
}

#[test]
fn borrowed_value_binding_retains_semantic_place_and_exact_storage() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
    };
    use semantic_vocabulary::PlaceId;
    let mut original = successor();
    original
        .structural_bindings
        .push(SelectedStructuralBinding {
            source_indices: Vec::new(),
            semantic: abstract_operations::AbstractStructuralBinding {
                parameter: PlaceId::new(31).unwrap(),
                argument: terminal_psi::StructuralArgument {
                    place: PlaceId::new(37).unwrap(),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            },
            transport: SelectedStructuralTransport::WholeValue {
                argument: VirtualRegisterId(41),
                source_byte_offset: 0,
                destination: LocalStorageSlotId::StructuralBlockParameter {
                    block: original.source_target,
                    place: PlaceId::new(31).unwrap(),
                },
                byte_size: 16,
                alignment: 8,
            },
        });
    let mut stored_bytes = Vec::new();
    encode_successor(&mut stored_bytes, &original);
    let mut unused = original.clone();
    unused.structural_bindings[0].transport = SelectedStructuralTransport::Unused;
    let mut unused_bytes = Vec::new();
    encode_successor(&mut unused_bytes, &unused);
    let transport_offset = stored_bytes
        .iter()
        .zip(&unused_bytes)
        .position(|(stored, unused)| stored != unused)
        .unwrap();
    assert_eq!(stored_bytes[transport_offset], 2);
    assert_eq!(unused_bytes[transport_offset], 0);
    // The retired descriptor tag must not silently acquire whole-value semantics.
    stored_bytes[transport_offset] = 1;
    assert_eq!(
        decode_successor(&mut Cursor::new(&stored_bytes)),
        Err(FixedViewCopyDecodeError::UnknownValueTransport(1))
    );
    for transport in [
        original.structural_bindings[0].transport,
        SelectedStructuralTransport::Unused,
    ] {
        original.structural_bindings[0].transport = transport;
        let mut bytes = Vec::new();
        encode_successor(&mut bytes, &original);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_successor(&mut Cursor::new(&bytes[..length])).is_err());
        }
    }
}
#[test]
fn successor_rejects_unknown_transport_and_incomplete_register_pair() {
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &successor());
    encoded[46] = 2;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownValueTransport(2))
    );
    encoded[46] = 1;
    for length in 47..55 {
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded[..length])),
            Err(FixedViewCopyDecodeError::Truncated)
        );
    }
}

#[test]
fn successor_role_round_trips_and_unknown_roles_reject() {
    let mut original = successor();
    original.role = SelectedSuccessorRole::EdgeTransferContinuation;
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    assert_eq!(encoded[0], 1);
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)).unwrap(),
        original
    );
    original.role = SelectedSuccessorRole::CaseDispatchContinuation;
    encoded.clear();
    encode_successor(&mut encoded, &original);
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)).unwrap(),
        original
    );
    encoded[0] = 3;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownSuccessorRole(3))
    );
}

#[test]
fn implementation_block_origin_round_trips_without_a_fabricated_source_block() {
    let mut block =
        crate::register_homes::recovery::fixed_view_copy::codec::tests::function_with_operand(
            register_model::RegisterOperandAccess::Use,
        )
        .blocks
        .remove(0);
    block.origin = SelectedBlockOrigin::EdgeTransfer {
        edge: EdgeId::new(71).unwrap(),
        target: BlockId::new(91).unwrap(),
    };
    let mut encoded = Vec::new();
    encode_block(&mut encoded, &block);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_block(&mut cursor).unwrap(), block);
    assert_eq!(cursor.remaining(), 0);
    block.origin = SelectedBlockOrigin::CaseDispatch {
        source: BlockId::new(91).unwrap(),
        case_ordinal: 2,
    };
    encoded.clear();
    encode_block(&mut encoded, &block);
    assert_eq!(decode_block(&mut Cursor::new(&encoded)).unwrap(), block);
    encoded[4] = 3;
    assert_eq!(
        decode_block(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownBlockOrigin(3))
    );
}

#[test]
fn case_payload_codec_retains_every_semantic_and_transport_field() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedCaseDispatchSource, SelectedCasePayloadBinding,
        SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
    };
    use semantic_vocabulary::{OperationId, PlaceId, StructuralCaseId, StructuralFieldId};
    use terminal_psi::TerminalAffineCleanupAction;
    let mut original = successor();
    original.structural_case = Some(SelectedStructuralCaseEdge {
        source: SelectedCaseDispatchSource::Local {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(7).unwrap(),
                place: PlaceId::new(11).unwrap(),
            },
        },
        case: StructuralCaseId::new(2).unwrap(),
        case_tag: 1,
        payloads: vec![SelectedCasePayloadBinding {
            semantic: legalized_operations::LegalizedStructuralCasePayload {
                field: StructuralFieldId::new(3).unwrap(),
                field_byte_offset: 4,
                parameter: legalized_operations::LegalizedValueDefinition {
                    value: ValueId::new(9).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                        block: original.source_target,
                        position: 1,
                    },
                },
            },
            transport: SelectedCasePayloadTransport::Unused,
        }],
        cleanup_actions: vec![
            TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(11).unwrap()),
            TerminalAffineCleanupAction::DiscardResidual(terminal_psi::StructuralAffineDiscard {
                place: PlaceId::new(12).unwrap(),
                path: vec![terminal_psi::StructuralPathSegment::Field(
                    terminal_psi::TerminalIdentityIndex(1),
                )],
                structural_type: semantic_vocabulary::StructuralTypeId::new(4).unwrap(),
            }),
        ],
    });
    let mut encodings = Vec::new();
    for transport in [
        SelectedCasePayloadTransport::Unused,
        SelectedCasePayloadTransport::Unmaterialized {
            parameter: VirtualRegisterId(29),
        },
        SelectedCasePayloadTransport::Registers {
            argument: VirtualRegisterId(17),
            parameter: VirtualRegisterId(29),
        },
    ] {
        original.structural_case.as_mut().unwrap().payloads[0].transport = transport;
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)).unwrap(),
            original
        );
        for end in 0..encoded.len() {
            assert!(decode_successor(&mut Cursor::new(&encoded[..end])).is_err());
        }
        encodings.push(encoded);
    }
    assert_ne!(encodings[0], encodings[1]);
    assert_ne!(encodings[1], encodings[2]);
    for mutation in 0..7 {
        let mut changed = original.clone();
        let case = changed.structural_case.as_mut().unwrap();
        match mutation {
            0 => case.case_tag = 0,
            1 => case.payloads[0].semantic.field_byte_offset += 4,
            2 => case.payloads[0].semantic.parameter.value = ValueId::new(10).unwrap(),
            3 => case
                .cleanup_actions
                .retain(|action| !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_))),
            6 => case.cleanup_actions.retain(|action| {
                !matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
            }),
            4 => {
                case.source = SelectedCaseDispatchSource::Local {
                    slot: LocalStorageSlotId::Boundary {
                        operation: OperationId::new(7).unwrap(),
                    },
                }
            }
            _ => {
                case.payloads[0].semantic.parameter.definition_site =
                    optimization_unit::ValueDefinitionSite::FunctionParameter(1)
            }
        }
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &changed);
        assert_ne!(encoded, encodings[2], "mutation {mutation}");
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)).unwrap(),
            changed
        );
    }
}

#[test]
fn whole_value_transport_round_trips_exact_storage_and_rejects_invalid_extents() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
    };
    let mut original = successor();
    original
        .structural_bindings
        .push(SelectedStructuralBinding {
            source_indices: Vec::new(),
            semantic: abstract_operations::AbstractStructuralBinding {
                parameter: semantic_vocabulary::PlaceId::new(12).unwrap(),
                argument: terminal_psi::StructuralArgument {
                    place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                    access: terminal_psi::StructuralAccess::Owned,
                    path: Vec::new(),
                },
            },
            transport: SelectedStructuralTransport::WholeValue {
                argument: VirtualRegisterId(30),
                source_byte_offset: 16,
                destination: LocalStorageSlotId::StructuralBlockParameter {
                    block: original.source_target,
                    place: semantic_vocabulary::PlaceId::new(12).unwrap(),
                },
                byte_size: 12,
                alignment: 4,
            },
        });
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_successor(&mut cursor).unwrap(), original);
    assert_eq!(cursor.remaining(), 0);
    for (byte_size, alignment, source_offset) in [
        (0, 4, 16),
        (12, 0, 16),
        (12, 3, 16),
        (12, 8, 16),
        (12, 4, 1),
        (12, 4, u32::MAX - 3),
    ] {
        let mut forged = original.clone();
        let SelectedStructuralTransport::WholeValue {
            byte_size: bytes,
            alignment: align,
            source_byte_offset,
            ..
        } = &mut forged.structural_bindings[0].transport
        else {
            unreachable!()
        };
        *bytes = byte_size;
        *align = alignment;
        *source_byte_offset = source_offset;
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &forged);
        assert_eq!(
            decode_successor(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
        );
    }
    let mut forged = original.clone();
    let SelectedStructuralTransport::WholeValue { destination, .. } =
        &mut forged.structural_bindings[0].transport
    else {
        unreachable!()
    };
    *destination = LocalStorageSlotId::Structural {
        operation: semantic_vocabulary::OperationId::new(7).unwrap(),
        place: semantic_vocabulary::PlaceId::new(12).unwrap(),
    };
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &forged);
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
    );
}

#[test]
fn address_transport_round_trips_both_bases_and_rejects_forged_carriers() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedAddressBase, SelectedStructuralBinding,
        SelectedStructuralTransport,
    };
    let join = semantic_vocabulary::PlaceId::new(12).unwrap();
    for base in [
        SelectedAddressBase::Register(VirtualRegisterId(30)),
        SelectedAddressBase::Local(LocalStorageSlotId::Structural {
            operation: semantic_vocabulary::OperationId::new(7).unwrap(),
            place: semantic_vocabulary::PlaceId::new(11).unwrap(),
        }),
    ] {
        let mut original = successor();
        original
            .structural_bindings
            .push(SelectedStructuralBinding {
                source_indices: Vec::new(),
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: join,
                    argument: terminal_psi::StructuralArgument {
                        place: semantic_vocabulary::PlaceId::new(11).unwrap(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                        path: vec![terminal_psi::StructuralPathSegment::Field(
                            terminal_psi::TerminalIdentityIndex(2),
                        )],
                    },
                },
                transport: SelectedStructuralTransport::Address {
                    base,
                    byte_offset: 8,
                    byte_count: 8,
                    destination: LocalStorageSlotId::StructuralBlockParameter {
                        block: original.source_target,
                        place: join,
                    },
                },
            });
        let mut encoded = Vec::new();
        encode_successor(&mut encoded, &original);
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_successor(&mut cursor).unwrap(), original);
        assert_eq!(cursor.remaining(), 0);
        // An empty lent span or a carrier outside the join's own block slot
        // is not an address transport.
        for mutation in 0..2 {
            let mut forged = original.clone();
            let SelectedStructuralTransport::Address {
                byte_count,
                destination,
                ..
            } = &mut forged.structural_bindings[0].transport
            else {
                unreachable!()
            };
            if mutation == 0 {
                *byte_count = 0;
            } else {
                *destination = LocalStorageSlotId::Structural {
                    operation: semantic_vocabulary::OperationId::new(7).unwrap(),
                    place: join,
                };
            }
            let mut encoded = Vec::new();
            encode_successor(&mut encoded, &forged);
            assert_eq!(
                decode_successor(&mut Cursor::new(&encoded)),
                Err(FixedViewCopyDecodeError::InvalidStructuralTransport)
            );
        }
    }
}

#[test]
fn successor_case_round_trip_retains_ordered_cleanup_actions() {
    use selected_instructions::{SelectedCaseDispatchSource, SelectedStructuralCaseEdge};
    use semantic_vocabulary::{
        ClaimId, MachineId, ObligationId, PlaceId, StructuralCaseId, StructuralTypeId,
    };
    use terminal_psi::{
        NominalAffineCleanup, StructuralAffineDiscard, StructuralPathSegment,
        TerminalAffineCleanupAction,
    };

    let mut original = successor();
    original.structural_case = Some(SelectedStructuralCaseEdge {
        source: SelectedCaseDispatchSource::Borrowed {
            place: PlaceId::new(10).unwrap(),
            pointer: VirtualRegisterId(11),
            byte_size: 16,
        },
        case: StructuralCaseId::new(12).unwrap(),
        case_tag: 3,
        payloads: Vec::new(),
        cleanup_actions: vec![
            TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(13).unwrap()),
            TerminalAffineCleanupAction::InvokeNominal(NominalAffineCleanup {
                retains_descendants: false,
                place: PlaceId::new(14).unwrap(),
                path: vec![
                    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(3)),
                    StructuralPathSegment::FixedIndex(1),
                ],
                structural_type: StructuralTypeId::new(15).unwrap(),
                cleanup_machine: MachineId::new(16).unwrap(),
                consumed_claims: vec![ClaimId::new(17).unwrap()],
                cleanup_receiver: Some(PlaceId::new(18).unwrap()),
                requirement_obligations: vec![ObligationId::new(19).unwrap()],
            }),
            TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                place: PlaceId::new(20).unwrap(),
                structural_type: StructuralTypeId::new(21).unwrap(),
                path: vec![
                    StructuralPathSegment::Case(StructuralCaseId::new(22).unwrap()),
                    StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(4)),
                    StructuralPathSegment::Referent,
                    StructuralPathSegment::FixedIndex(1),
                    StructuralPathSegment::FixedByteRange { start: 2, end: 4 },
                    StructuralPathSegment::RuntimeIndex {
                        index: ValueId::new(23).unwrap(),
                        obligation: ObligationId::new(24).unwrap(),
                    },
                ],
            }),
        ],
    });
    let mut encoded = Vec::new();
    encode_successor(&mut encoded, &original);
    let mut cursor = Cursor::new(&encoded);
    assert_eq!(decode_successor(&mut cursor).unwrap(), original);
    assert_eq!(cursor.remaining(), 0);
}
