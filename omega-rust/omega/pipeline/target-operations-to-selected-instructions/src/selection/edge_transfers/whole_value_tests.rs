//! Whole-value edge mechanics, independently exercised with swaps and narrow tails.
use super::*;
use selected_instructions::{FrameStorageSlotId, SelectedStructuralTransport};

#[test]
fn whole_values_snapshot_before_parallel_replacement_and_replay_exact_extents() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for width in [1u16, 2, 4, 8, 12, 16] {
            let mut original =
                super::descriptor_tests::swapped_function(&constraints, environment.constraints());
            let alignment = if width == 12 { 4 } else { width.min(8) };
            for slot in &mut original.local_storage_slots {
                slot.byte_size = u32::from(width);
                slot.alignment = alignment;
            }
            for memory in &mut original.memory_accesses {
                memory.byte_count = u32::from(width);
            }
            let SelectedTerminator::Jump { successor, .. } = &mut original.blocks[0].terminator
            else {
                unreachable!()
            };
            for binding in &mut successor.structural_bindings {
                let SelectedStructuralTransport::Descriptor {
                    argument,
                    destination,
                } = binding.transport
                else {
                    unreachable!()
                };
                binding.semantic.argument.access = terminal_psi::StructuralAccess::Owned;
                binding.transport = SelectedStructuralTransport::WholeValue {
                    argument,
                    destination,
                    byte_size: width,
                    alignment,
                };
            }
            let mut prepared = original.clone();
            prepare(0, &mut prepared, &constraints, environment.constraints()).unwrap();
            assert_eq!(project(0, &prepared, &constraints).unwrap(), original);
            let bridge = &prepared.blocks[1];
            let slots = &original.local_storage_slots;
            let mut values = vec![0u64; prepared.virtual_registers.len()];
            values[..4].copy_from_slice(&[11, 22, 1000, 2000]);
            let mut storage = [
                vec![0x35u8; usize::from(width)],
                vec![0xa7u8; usize::from(width)],
            ];
            for instruction in &bridge.instructions {
                let operand =
                    |position: usize| instruction.operands[position].virtual_register.0 as usize;
                match instruction.kind {
                    SelectedInstructionKind::CopyI64 => values[operand(1)] = values[operand(0)],
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    } => {
                        let position = slots
                            .iter()
                            .position(|candidate| candidate.id == slot)
                            .unwrap();
                        values[operand(0)] = if position == 0 { 1000 } else { 2000 };
                    }
                    SelectedInstructionKind::Load64 { byte_offset }
                    | SelectedInstructionKind::Load32 { byte_offset }
                    | SelectedInstructionKind::Load16 { byte_offset }
                    | SelectedInstructionKind::Load8 { byte_offset } => {
                        let count = match instruction.kind {
                            SelectedInstructionKind::Load64 { .. } => 8,
                            SelectedInstructionKind::Load32 { .. } => 4,
                            SelectedInstructionKind::Load16 { .. } => 2,
                            _ => 1,
                        };
                        let source = match values[operand(0)] {
                            1000 => 0,
                            2000 => 1,
                            _ => panic!("source address"),
                        };
                        let mut bytes = [0u8; 8];
                        bytes[..count].copy_from_slice(
                            &storage[source][byte_offset as usize..byte_offset as usize + count],
                        );
                        values[operand(1)] = u64::from_le_bytes(bytes);
                    }
                    SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    } => {
                        let destination = match values[operand(0)] {
                            1000 => 0,
                            2000 => 1,
                            _ => panic!("destination address"),
                        };
                        let bytes = values[operand(1)].to_le_bytes();
                        storage[destination]
                            [byte_offset as usize..byte_offset as usize + usize::from(byte_size)]
                            .copy_from_slice(&bytes[..usize::from(byte_size)]);
                    }
                    _ => panic!("unexpected copy instruction"),
                }
                assert!(instruction.provenance.fuel.is_empty());
            }
            assert_eq!(
                storage,
                [
                    vec![0xa7; usize::from(width)],
                    vec![0x35; usize::from(width)]
                ]
            );
            let SelectedTerminator::Jump { successor, .. } = &bridge.terminator else {
                unreachable!()
            };
            assert!(successor.fuel.is_empty());
            for mutation in 0..7 {
                let mut forged = prepared.clone();
                let block = &mut forged.blocks[1];
                let first_store = block
                    .instructions
                    .iter()
                    .position(|instruction| {
                        matches!(instruction.kind, SelectedInstructionKind::Store { .. })
                    })
                    .unwrap();
                match mutation {
                    0 => {
                        block.instructions.pop();
                    }
                    1 => block.instructions.swap(2, first_store),
                    2 => block.instructions[2].operands[0].virtual_register = VirtualRegisterId(2),
                    3 => {
                        block.instructions[first_store].kind = SelectedInstructionKind::Store {
                            byte_offset: 1,
                            byte_size: 1,
                        }
                    }
                    4 => forged.memory_accesses.last_mut().unwrap().byte_count += 1,
                    5 => {
                        let SelectedTerminator::Jump { successor, .. } = &mut block.terminator
                        else {
                            unreachable!()
                        };
                        let SelectedStructuralTransport::WholeValue { byte_size, .. } =
                            &mut successor.structural_bindings[0].transport
                        else {
                            unreachable!()
                        };
                        *byte_size += 1;
                    }
                    _ => {
                        let SelectedTerminator::Jump { successor, .. } = &mut block.terminator
                        else {
                            unreachable!()
                        };
                        successor.fuel = vec![FuelSettlement {
                            site: PsiProvenance::Edge(successor.psi_edge),
                            units: 1,
                        }];
                    }
                }
                assert!(
                    project(0, &forged, &constraints).is_err(),
                    "mutation {mutation}, width {width}, {target:?}"
                );
            }
        }
    }
}
