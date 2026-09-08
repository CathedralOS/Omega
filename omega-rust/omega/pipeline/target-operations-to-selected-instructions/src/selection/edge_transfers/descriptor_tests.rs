//! Transfer mechanics only: no claim of source cycle admission or native execution.
use super::*;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedStructuralBinding,
    SelectedStructuralTransport, SelectedValueBinding,
};
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, MachineId, PlaceId};

#[test]
fn scalar_and_descriptor_swaps_snapshot_all_inputs_before_reentry_replacement() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        let class =
            super::super::constraints::row(environment.constraints(), constraints.keys.copy_i64)
                .unwrap()
                .operands[0]
                .class;
        let block = BlockId::new(1).unwrap();
        let edge = EdgeId::new(1).unwrap();
        let places = [PlaceId::new(10).unwrap(), PlaceId::new(11).unwrap()];
        let slots =
            places.map(|place| LocalStorageSlotId::StructuralBlockParameter { block, place });
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let mut registers = (0..2)
            .map(|position| VirtualRegister {
                id: VirtualRegisterId(position),
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::BlockParameter {
                    source_value: ValueId::new(u64::from(position) + 1).unwrap(),
                    block: SelectedBlockId(0),
                    parameter_index: position as usize,
                },
                definition_site: Some(ValueDefinitionSite::BlockParameter { block, position }),
                entry_fixed_view: None,
            })
            .collect::<Vec<_>>();
        let mut instructions = Vec::new();
        let mut memory = Vec::new();
        for position in 0..2 {
            let instruction = SelectedInstructionId(position as u32);
            let pointer = VirtualRegisterId(position as u32 + 2);
            registers.push(VirtualRegister {
                id: pointer,
                scalar_type,
                class,
                origin: VirtualRegisterOrigin::AbiTransport {
                    instruction,
                    place: places[position],
                    byte_offset: 0,
                },
                definition_site: None,
                entry_fixed_view: None,
            });
            instructions.push(
                super::super::constraints::instruction(
                    instruction,
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slots[position]),
                        byte_offset: 0,
                    },
                    constraints.keys.frame_address.unwrap(),
                    &[pointer],
                    SelectedInstructionProvenance::default(),
                    environment.constraints(),
                )
                .unwrap(),
            );
            memory.push(SelectedMemoryAccess {
                instruction,
                origin: SelectedMemoryAccessOrigin::Block(block),
                place: places[position],
                byte_offset: 0,
                byte_count: 16,
                role: SelectedMemoryAccessRole::AddressLocal {
                    slot: slots[position],
                },
            });
        }
        let bindings = (0..2)
            .map(|position| SelectedValueBinding {
                semantic: abstract_operations::ValueBinding {
                    parameter: ValueId::new(u64::from(position) + 1).unwrap(),
                    argument: ValueId::new(u64::from(1 - position) + 1).unwrap(),
                    scalar_type,
                },
                transport: SelectedValueTransport::Registers {
                    argument: VirtualRegisterId(1 - position),
                    parameter: VirtualRegisterId(position),
                },
            })
            .collect();
        let structural_bindings = (0..2)
            .map(|position| SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: places[position],
                    argument: terminal_psi::StructuralArgument {
                        place: places[1 - position],
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                transport: SelectedStructuralTransport::Descriptor {
                    argument: VirtualRegisterId((3 - position) as u32),
                    destination: slots[position],
                },
            })
            .collect();
        let jump = super::super::constraints::instruction(
            SelectedInstructionId(2),
            SelectedInstructionKind::Jump,
            constraints.keys.jump,
            &[],
            SelectedInstructionProvenance::default(),
            environment.constraints(),
        )
        .unwrap();
        let original = SelectedFunction {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: target_operations::TerminalPsiProvenance {
                operations: Vec::new(),
                edges: vec![edge],
            },
            ranked: None,
            structural: None,
            local_storage_slots: slots
                .map(|id| SelectedLocalStorageSlot {
                    id,
                    byte_size: 16,
                    alignment: 8,
                })
                .to_vec(),
            outgoing_arguments: Vec::new(),
            calls: Vec::new(),
            memory_accesses: memory,
            boundary_settlements: Vec::new(),
            entry_block: SelectedBlockId(0),
            virtual_registers: registers,
            blocks: vec![SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(block),
                instructions,
                terminator: SelectedTerminator::Jump {
                    instruction: jump,
                    successor: SelectedSuccessor {
                        structural_case: None,
                        role: SelectedSuccessorRole::Semantic,
                        psi_edge: edge,
                        block: SelectedBlockId(0),
                        source_target: block,
                        bindings,
                        structural_bindings,
                        fuel: vec![FuelSettlement {
                            site: PsiProvenance::Edge(edge),
                            units: 1,
                        }],
                    },
                },
            }],
        };
        let mut prepared = original.clone();
        prepare(0, &mut prepared, &constraints, environment.constraints()).unwrap();
        assert_eq!(project(0, &prepared, &constraints).unwrap(), original);
        let bridge = &prepared.blocks[1];
        let SelectedTerminator::Jump { successor, .. } = &bridge.terminator else {
            panic!("continuation");
        };
        assert!(successor.fuel.is_empty());
        let mut homes = (0..prepared.virtual_registers.len()).collect::<Vec<_>>();
        for binding in &successor.bindings {
            let SelectedValueTransport::Registers {
                argument,
                parameter,
            } = binding.transport
            else {
                panic!("scalar transport");
            };
            homes[argument.0 as usize] = homes[parameter.0 as usize];
        }
        // Other homes are distinct; final scalar outputs occupy their destination
        // homes. Descriptor pointers refer to the very slots overwritten below.
        let mut physical = vec![0_u64; prepared.virtual_registers.len()];
        physical[..4].copy_from_slice(&[11, 22, 1000, 2000]);
        let mut descriptors = [[0x1000_u64, 3], [0x8000_u64, 17]];
        for iteration in 0..2 {
            for instruction in &bridge.instructions {
                let home = |position: usize| {
                    homes[instruction.operands[position].virtual_register.0 as usize]
                };
                match instruction.kind {
                    SelectedInstructionKind::CopyI64 => physical[home(1)] = physical[home(0)],
                    SelectedInstructionKind::Load64 { byte_offset } => {
                        let descriptor = match physical[home(0)] {
                            1000 => 0,
                            2000 => 1,
                            other => panic!("invalid descriptor {other}"),
                        };
                        physical[home(1)] = descriptors[descriptor][byte_offset as usize / 8];
                    }
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset,
                    } => {
                        let destination = slots
                            .iter()
                            .position(|candidate| *candidate == slot)
                            .unwrap();
                        descriptors[destination][byte_offset as usize / 8] = physical[home(0)];
                    }
                    _ => panic!("unexpected transfer instruction"),
                }
                assert!(instruction.provenance.fuel.is_empty());
            }
            assert_eq!(
                &physical[..2],
                if iteration == 0 { &[22, 11] } else { &[11, 22] }
            );
            assert_eq!(
                descriptors,
                if iteration == 0 {
                    [[0x8000, 17], [0x1000, 3]]
                } else {
                    [[0x1000, 3], [0x8000, 17]]
                }
            );
        }
        for mutation in 0..8 {
            let mut changed = prepared.clone();
            let bridge = &mut changed.blocks[1];
            match mutation {
                0 => bridge.instructions.swap(2, 8),
                1 => bridge.instructions[2].operands[0].virtual_register = VirtualRegisterId(2),
                2 => {
                    bridge.instructions[3].kind = SelectedInstructionKind::Load64 { byte_offset: 0 }
                }
                3 => {
                    bridge.instructions[8].kind = SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(slots[1]),
                        byte_offset: 0,
                    }
                }
                4 => {
                    bridge.origin = SelectedBlockOrigin::EdgeTransfer {
                        edge: EdgeId::new(9).unwrap(),
                        target: block,
                    }
                }
                5 => changed.memory_accesses.last_mut().unwrap().place = places[0],
                6 => {
                    let SelectedTerminator::Jump { successor, .. } = &mut bridge.terminator else {
                        panic!("jump");
                    };
                    successor.structural_bindings.pop();
                }
                _ => bridge
                    .instructions
                    .push(bridge.instructions.last().unwrap().clone()),
            }
            assert_ne!(
                changed, prepared,
                "mutation {mutation} must change the plan"
            );
            assert!(
                project(0, &changed, &constraints).is_err(),
                "mutation {mutation}: {native:?}"
            );
        }
    }
}
