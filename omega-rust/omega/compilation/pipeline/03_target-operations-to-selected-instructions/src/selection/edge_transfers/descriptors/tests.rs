//! Structural emission bounds: large storage is not allocated by these tests.
use super::*;
use selected_instructions::{
    SelectedSelectionConstraints, SelectedStructuralTransport, VirtualRegister,
    VirtualRegisterOrigin,
};
use semantic_vocabulary::{BlockId, EdgeId};
use target::NativeTarget;

fn emitted(
    target: NativeTarget,
    byte_size: u32,
    source_offset: u32,
) -> (
    Vec<selected_instructions::SelectedInstruction>,
    Vec<SelectedMemoryAccess>,
    Vec<VirtualRegister>,
    Vec<SelectedLocalStorageSlot>,
) {
    let environment = register_environment::baseline_target_register_environment(target).unwrap();
    let constraints = SelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs: vec![],
    };
    let catalog = environment.constraints();
    let place = PlaceId::new(1).unwrap();
    let parameter = PlaceId::new(2).unwrap();
    let edge = EdgeId::new(3).unwrap();
    let destination = LocalStorageSlotId::StructuralBlockParameter {
        block: BlockId::new(4).unwrap(),
        place: parameter,
    };
    let binding = SelectedStructuralBinding {
        semantic: abstract_operations::AbstractStructuralBinding {
            parameter,
            argument: terminal_psi::StructuralArgument::whole(
                place,
                terminal_psi::StructuralAccess::Owned,
            ),
        },
        transport: SelectedStructuralTransport::WholeValue {
            argument: VirtualRegisterId(0),
            source_byte_offset: source_offset,
            destination,
            byte_size,
            alignment: 16,
        },
        source_indices: vec![],
    };
    let class = crate::selection::constraints::row(catalog, constraints.keys.materialize_i64)
        .unwrap()
        .operands[0]
        .class;
    let mut registers = vec![VirtualRegister {
        id: VirtualRegisterId(0),
        scalar_type: super::super::emission::pointer_type(0).unwrap(),
        class,
        origin: VirtualRegisterOrigin::AbiTransport {
            instruction: selected_instructions::SelectedInstructionId(99),
            place,
            byte_offset: source_offset,
        },
        definition_site: None,
        entry_fixed_view: None,
    }];
    let mut instructions = vec![];
    let mut memory = vec![];
    let mut next_instruction = 0;
    let mut slots = vec![];
    let mut emission = Emission {
        function: 0,
        registers: &mut registers,
        memory: &mut memory,
        instructions: &mut instructions,
        next_instruction: &mut next_instruction,
        edge,
        environment: &environment,
        constraints: &constraints,
        catalog,
    };
    let snapshots = snapshot(&mut emission, &mut slots, std::slice::from_ref(&binding)).unwrap();
    let capture_end = emission.instructions.len();
    store(&mut emission, std::slice::from_ref(&binding), &snapshots).unwrap();
    assert!(
        emission
            .memory
            .iter()
            .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
            .all(|access| access.instruction.0 < capture_end as u32)
    );
    assert!(
        emission
            .memory
            .iter()
            .filter(|access| matches!(access.role, SelectedMemoryAccessRole::WriteLocal { .. }))
            .all(|access| access.instruction.0 >= capture_end as u32)
    );
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].byte_size, byte_size.max(1));
    assert_eq!(slots[0].alignment, 16);
    assert_eq!(
        slots[0].id,
        LocalStorageSlotId::EdgeSnapshot {
            edge,
            binding_index: 0
        }
    );
    (instructions, memory, registers, slots)
}

#[test]
fn edge_copy_zero_small_tail_and_bulk_boundary_emit_exact_extents() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for (size, chunks) in [(0, 0), (1, 1), (15, 4), (4096, 512)] {
            let (instructions, memory, _, _) = emitted(target, size, 0);
            assert_eq!(instructions.len(), 2 + 4 * chunks);
            assert!(
                !instructions
                    .iter()
                    .any(|instruction| instruction.kind == SelectedInstructionKind::CopyBytes)
            );
            let reads: Vec<_> = memory
                .iter()
                .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                .collect();
            let writes: Vec<_> = memory
                .iter()
                .filter(|access| matches!(access.role, SelectedMemoryAccessRole::WriteLocal { .. }))
                .collect();
            assert_eq!(reads.len(), chunks);
            assert_eq!(writes.len(), chunks);
            assert_eq!(
                reads.iter().map(|access| access.byte_count).sum::<u32>(),
                size
            );
            assert_eq!(
                writes.iter().map(|access| access.byte_count).sum::<u32>(),
                size
            );
            for (read, write) in reads.iter().zip(writes) {
                assert_eq!(
                    (read.byte_offset, read.byte_count),
                    (write.byte_offset, write.byte_count)
                );
            }
        }
        let (instructions, memory, _, _) = emitted(target, 4097, 0);
        assert_eq!(instructions.len(), 6);
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| instruction.kind == SelectedInstructionKind::CopyBytes)
                .count(),
            2
        );
        assert_eq!(
            memory
                .iter()
                .filter(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                .map(|access| access.byte_count)
                .collect::<Vec<_>>(),
            [4097]
        );
    }
}

#[test]
fn huge_edge_copy_emission_is_bounded_and_retains_offset_count_and_private_homes() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for size in [5_242_880, u32::MAX - 17] {
            let (instructions, memory, registers, _) = emitted(target, size, 17);
            assert_eq!(instructions.len(), 7);
            assert_eq!(registers.len(), 10);
            assert!(instructions.iter().any(|instruction| instruction.kind
                == SelectedInstructionKind::AddressOffset { byte_offset: 17 }));
            let copies: Vec<_> = instructions
                .iter()
                .filter(|instruction| instruction.kind == SelectedInstructionKind::CopyBytes)
                .collect();
            assert_eq!(copies.len(), 2);
            for copy in copies {
                let count = copy.operands[2].virtual_register;
                assert!(instructions.iter().any(|instruction| instruction.kind
                    == SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(u128::from(size))
                    }
                    && instruction.operands[0].virtual_register == count));
                for operand in &copy.operands[3..] {
                    assert!(operand.early_clobber);
                    assert_eq!(
                        registers[operand.virtual_register.0 as usize].origin,
                        VirtualRegisterOrigin::InstructionScratch {
                            instruction: copy.id,
                            operand: operand.operand
                        }
                    );
                }
            }
            let read = memory
                .iter()
                .find(|access| access.role == SelectedMemoryAccessRole::ReadPlace)
                .unwrap();
            assert_eq!((read.byte_offset, read.byte_count), (17, size));
            let write = memory
                .iter()
                .find(|access| matches!(access.role, SelectedMemoryAccessRole::WriteLocal { .. }))
                .unwrap();
            assert_eq!((write.byte_offset, write.byte_count), (0, size));
            assert_eq!(memory.len(), 3); // source read, destination address and destination write only
        }
    }
}
