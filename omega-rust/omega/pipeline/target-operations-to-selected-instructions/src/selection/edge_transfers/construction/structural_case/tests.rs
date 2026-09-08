//! Raw edge construction checks, not source-admission or native execution claims.
use super::*;
use selected_instructions::{
    LocalStorageSlotId, SelectedCasePayloadBinding, SelectedStructuralCaseEdge,
};
use semantic_vocabulary::{
    BlockId, EdgeId, OperationId, PlaceId, StructuralCaseId, StructuralFieldId, ValueId,
};

#[test]
fn case_payload_bridge_snapshots_each_used_field_before_destination_binding() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let class = crate::selection::constraints::row(
            environment.constraints(),
            constraints.keys.materialize_i64,
        )
        .unwrap()
        .operands[0]
            .class;
        let block = BlockId::new(20).unwrap();
        let place = PlaceId::new(3).unwrap();
        let edge = EdgeId::new(4).unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(2).unwrap(),
            place,
        };
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let mut registers = Vec::new();
        let payloads = (0..3)
            .map(|position| {
                let parameter = VirtualRegisterId(position);
                let value = ValueId::new(u64::from(position + 1)).unwrap();
                let site = ValueDefinitionSite::BlockParameter { block, position };
                registers.push(VirtualRegister {
                    id: parameter,
                    scalar_type,
                    class,
                    origin: VirtualRegisterOrigin::BlockParameter {
                        source_value: value,
                        block: SelectedBlockId(1),
                        parameter_index: position as usize,
                    },
                    definition_site: Some(site),
                    entry_fixed_view: None,
                });
                SelectedCasePayloadBinding {
                    semantic: legalized_operations::LegalizedStructuralCasePayload {
                        field: StructuralFieldId::new(1).unwrap(),
                        field_byte_offset: 4,
                        parameter: legalized_operations::LegalizedValueDefinition {
                            value,
                            scalar_type,
                            definition_site: site,
                        },
                    },
                    transport: if position == 1 {
                        SelectedCasePayloadTransport::Unused
                    } else {
                        SelectedCasePayloadTransport::Unmaterialized { parameter }
                    },
                }
            })
            .collect();
        let mut successor = SelectedSuccessor {
            role: SelectedSuccessorRole::Semantic,
            psi_edge: edge,
            block: SelectedBlockId(1),
            source_target: block,
            bindings: Vec::new(),
            structural_bindings: Vec::new(),
            structural_case: Some(SelectedStructuralCaseEdge {
                slot,
                case: StructuralCaseId::new(2).unwrap(),
                case_tag: 1,
                payloads,
                trivial_affine_discards: vec![place],
            }),
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(edge),
                units: 1,
            }],
        };
        let original = successor.clone();
        let slots = vec![SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 4,
        }];
        let mut memory = Vec::new();
        let mut next_instruction = 10;
        let bridge = prepare(
            0,
            &mut successor,
            2,
            &mut registers,
            &mut memory,
            &slots,
            &mut next_instruction,
            &constraints,
            environment.constraints(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(successor.block, SelectedBlockId(2));
        assert_eq!(successor.fuel, original.fuel);
        assert!(
            successor
                .structural_case
                .as_ref()
                .unwrap()
                .payloads
                .iter()
                .all(|payload| payload.transport == SelectedCasePayloadTransport::Unused)
        );
        assert!(matches!(
            bridge.instructions.as_slice(),
            [
                SelectedInstruction {
                    kind: SelectedInstructionKind::FrameAddress { .. },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::Load32 { byte_offset: 4 },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::FrameAddress { .. },
                    ..
                },
                SelectedInstruction {
                    kind: SelectedInstructionKind::Load32 { byte_offset: 4 },
                    ..
                }
            ]
        ));
        let SelectedTerminator::Jump {
            successor: continuation,
            ..
        } = &bridge.terminator
        else {
            panic!("bridge jump")
        };
        assert!(continuation.fuel.is_empty());
        assert!(
            continuation
                .structural_case
                .as_ref()
                .unwrap()
                .trivial_affine_discards
                .is_empty()
        );
        let payloads = &continuation.structural_case.as_ref().unwrap().payloads;
        assert_eq!(payloads[1].transport, SelectedCasePayloadTransport::Unused);
        for position in [0, 2] {
            let SelectedCasePayloadTransport::Registers {
                argument,
                parameter,
            } = payloads[position].transport
            else {
                panic!("payload transfer")
            };
            assert_eq!(parameter, VirtualRegisterId(position as u32));
            assert!(
                matches!(registers[argument.0 as usize].origin, VirtualRegisterOrigin::StructuralObservation { place: actual, byte_offset: 4, .. } if actual == place)
            );
            assert_eq!(registers[argument.0 as usize].definition_site, None);
        }
        assert_eq!(memory.len(), 4);
        assert!(bridge.instructions.iter().all(|instruction| {
            instruction.provenance.operations.is_empty()
                && instruction.provenance.values.is_empty()
                && instruction.provenance.fuel.is_empty()
                && instruction.provenance.edges == [edge]
        }));
        assert_eq!(next_instruction, 15);
    }
}
