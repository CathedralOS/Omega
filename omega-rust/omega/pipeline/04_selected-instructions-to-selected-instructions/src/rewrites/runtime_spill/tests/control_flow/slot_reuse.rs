//! Physical slot reuse across real control flow. The last-writer replay joins
//! predecessor writer states by union, so merges and back edges — not just one
//! block's instruction order — decide whether a declared spill slot can host a
//! second victim without a destructive interleave.

use super::{cfg_fixture, cyclic_parameter_fixture};
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, NativeTarget, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator, ValueId,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    baseline_target_register_environment, budget, selected_instruction_plan_identity,
};
use crate::{spill_selected_runtime_value, validate_runtime_spill};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedValueBinding,
    SelectedValueTransport,
};

/// Reuse through a loop: the first victim's store and reload live inside the
/// loop body while the second — the loop-carried parameter — stores on every
/// arrival including the back edge. Every path into the merge then ends on a
/// new store, and the incumbent's store inside the merge restores its own
/// bytes ahead of its reload, so the slot stays singular.
#[test]
fn loop_carried_windows_share_when_every_arrival_restores() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = cyclic_parameter_fixture(target, false);
    // The first victim is the loop-body result read on the back edge: its
    // store lands in the merge block and its reload pair in the back-edge
    // block, declaring the slot the parameter victim can later share.
    let first =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(5), &environment, budget())
            .unwrap();
    let slot = first.transformed().functions[0].local_storage_slots[0].id;
    assert_eq!(
        slot,
        LocalStorageSlotId::Spill {
            register: VirtualRegisterId(5),
        }
    );
    let second =
        spill_selected_runtime_value(&first, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &second.transformed().functions[0];
    // Reuse declared nothing: the incumbent's entry stays the only slot.
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        }]
    );
    // Every arrival — both entry edges and the back edge — stores the bound
    // argument into the shared slot right after its edge copy.
    for (block_index, position) in [(1usize, 1usize), (3, 1), (4, 3)] {
        assert!(matches!(
            function.blocks[block_index].instructions[position].kind,
            SelectedInstructionKind::Store64 { slot: named, .. }
                if named == FrameStorageSlotId::Local(slot)
        ));
    }
    validate_runtime_spill(
        &first,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        second.transformed().clone(),
    )
    .unwrap();
}

/// A candidate slot's address register may not ride an edge transport: loads
/// in the destination would escape the scan, so the sharing check cannot prove
/// them disjoint and the victim keeps a private slot.
#[test]
fn an_escaping_slot_address_keeps_the_victim_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let mut source = cfg_fixture(target);
    let incumbent = LocalStorageSlotId::Spill {
        register: VirtualRegisterId(50),
    };
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        let scalar_type = function.virtual_registers[0].scalar_type;
        let class = function.virtual_registers[0].class;
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        });
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(51),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::SpillAddress {
                instruction: SelectedInstructionId(90),
                register: VirtualRegisterId(50),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.blocks[1].instructions.push(admission::instruction(
            SelectedInstructionId(90),
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(incumbent),
                byte_offset: 0,
            },
            address,
            &[VirtualRegisterId(51)],
        ));
        // The address register leaves the block on the edge's value transport:
        // unverifiable reads may consume it in the destination.
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[1].terminator else {
            unreachable!()
        };
        successor.bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(2).unwrap(),
                argument: ValueId::new(9).unwrap(),
                scalar_type,
            },
            transport: SelectedValueTransport::Registers {
                argument: VirtualRegisterId(51),
                parameter: VirtualRegisterId(9),
            },
        });
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [
            SelectedLocalStorageSlot {
                id: incumbent,
                byte_size: 8,
                alignment: 8,
            },
            SelectedLocalStorageSlot {
                id: LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(1),
                },
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(1),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}
