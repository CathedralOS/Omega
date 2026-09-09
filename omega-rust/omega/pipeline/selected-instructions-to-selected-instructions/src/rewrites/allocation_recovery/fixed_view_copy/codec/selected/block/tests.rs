use super::*;
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
fn descriptor_binding_retains_semantic_place_and_transport_without_scalar_identity() {
    use selected_instructions::{
        LocalStorageSlotId, SelectedStructuralBinding, SelectedStructuralTransport,
    };
    use semantic_vocabulary::PlaceId;
    let mut original = successor();
    original
        .structural_bindings
        .push(SelectedStructuralBinding {
            semantic: abstract_operations::AbstractStructuralBinding {
                parameter: PlaceId::new(31).unwrap(),
                argument: terminal_psi::StructuralArgument {
                    place: PlaceId::new(37).unwrap(),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            },
            transport: SelectedStructuralTransport::Descriptor {
                argument: VirtualRegisterId(41),
                destination: LocalStorageSlotId::StructuralBlockParameter {
                    block: original.source_target,
                    place: PlaceId::new(31).unwrap(),
                },
            },
        });
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
    assert_eq!(decode_successor(&mut Cursor::new(&encoded)).unwrap(), original);
    encoded[0] = 3;
    assert_eq!(
        decode_successor(&mut Cursor::new(&encoded)),
        Err(FixedViewCopyDecodeError::UnknownSuccessorRole(3))
    );
}

#[test]
fn implementation_block_origin_round_trips_without_a_fabricated_source_block() {
    let mut block = crate::tests::function_with_operand(register_model::RegisterOperandAccess::Use)
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
        source: BlockId::new(91).unwrap(), case_ordinal: 2,
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
        LocalStorageSlotId, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
        SelectedStructuralCaseEdge,
    };
    use semantic_vocabulary::{OperationId, PlaceId, StructuralCaseId, StructuralFieldId};
    let mut original = successor();
    original.structural_case = Some(SelectedStructuralCaseEdge {
        slot: LocalStorageSlotId::Structural {
            operation: OperationId::new(7).unwrap(),
            place: PlaceId::new(11).unwrap(),
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
        trivial_affine_discards: vec![PlaceId::new(11).unwrap()],
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
    for mutation in 0..6 {
        let mut changed = original.clone();
        let case = changed.structural_case.as_mut().unwrap();
        match mutation {
            0 => case.case_tag = 0,
            1 => case.payloads[0].semantic.field_byte_offset += 4,
            2 => case.payloads[0].semantic.parameter.value = ValueId::new(10).unwrap(),
            3 => case.trivial_affine_discards.clear(),
            4 => {
                case.slot = LocalStorageSlotId::Boundary {
                    operation: OperationId::new(7).unwrap(),
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
