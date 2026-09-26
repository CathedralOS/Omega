//! Physical slot reuse across real control flow. The last-writer replay joins
//! predecessor writer states by union, so merges and back edges — not just one
//! block's instruction order — decide whether a declared spill slot can host a
//! second victim without a destructive interleave.

use super::super::parameters::parameter_fixture;
use super::{cfg_fixture, cyclic_parameter_fixture};
use crate::rewrites::runtime_spill::admission;
use crate::rewrites::runtime_spill::tests::{
    Arc, NativeTarget, SelectedInstructionId, SelectedInstructionKind, SelectedTerminator,
    ValueDefinitionSite, ValueId, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    baseline_target_register_environment, budget, selected_instruction_plan_identity,
};
use crate::{spill_selected_runtime_value, validate_runtime_spill};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot, SelectedValueBinding,
    SelectedValueTransport,
};

/// One instruction-result victim defined in the entry block of the parameter
/// diamond and read once at the join: its store dominates both arms, but each
/// arm can leave a different last writer for a shared slot.
fn diamond_instruction_victim(target: NativeTarget) -> crate::ValidatedRuntimeSpill {
    let mut source = parameter_fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
    let scalar_type = function.virtual_registers[0].scalar_type;
    let class = function.virtual_registers[0].class;
    for (register, instruction) in [(8u32, 99u32), (9, 403)] {
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(instruction),
                source_value: ValueId::new(1).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
    }
    let copy_instruction = |identity, input, output| {
        admission::instruction(
            SelectedInstructionId(identity),
            SelectedInstructionKind::CopyI64,
            copy,
            &[VirtualRegisterId(input), VirtualRegisterId(output)],
        )
    };
    function.blocks[0]
        .instructions
        .push(copy_instruction(99, 0, 8));
    function.blocks[2]
        .instructions
        .push(copy_instruction(403, 8, 9));
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    source
}

/// A merge where the arms disagree about the last writer keeps the candidate
/// slot private: the incumbent's store ends the first arm while the second arm
/// still carries the new victim's store, so the join's reload could observe
/// either value. Union, not first-predecessor order, must supply that
/// rejection — the victim appends its own slot.
#[test]
fn merged_writers_at_a_join_keep_the_victim_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let store = environment
        .constraint(environment.selected_keys().store64.unwrap())
        .unwrap();
    let mut source = diamond_instruction_victim(target);
    let incumbent = LocalStorageSlotId::Spill {
        register: VirtualRegisterId(50),
    };
    {
        let function = &mut Arc::make_mut(&mut source.transformed).functions[0];
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        });
        // The incumbent's only access is a store ending the first arm.
        function.blocks[1].instructions.push(admission::instruction(
            SelectedInstructionId(203),
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(incumbent),
                byte_offset: 0,
            },
            store,
            &[VirtualRegisterId(0)],
        ));
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(8), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    // The merge refused the candidate: the victim's private slot is appended
    // behind the incumbent's declaration.
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
                    register: VirtualRegisterId(8),
                },
                byte_size: 8,
                alignment: 8,
            },
        ]
    );
    // The incumbent's store kept its slot while the victim's own store — at
    // the head of the shared block — names the fresh private slot.
    assert_eq!(
        function.blocks[1].instructions[2].kind,
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(incumbent),
            byte_offset: 0,
        }
    );
    assert_eq!(
        function.blocks[0].instructions[1].kind,
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                register: VirtualRegisterId(8),
            }),
            byte_offset: 0,
        }
    );
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(8),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The same diamond admits sharing once the incumbent's window closes before
/// the victim's store: the entry block's incumbent store and load complete
/// ahead of the victim's definition, so both arms — and the join — see only
/// the new writer.
#[test]
fn a_window_closed_before_the_join_stays_shareable() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let store = environment.constraint(keys.store64.unwrap()).unwrap();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let mut source = diamond_instruction_victim(target);
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
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(52),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(91),
                source_value: ValueId::new(9).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        let frame_slot = FrameStorageSlotId::Local(incumbent);
        // The incumbent's whole window — store, address, load — sits ahead of
        // the victim's own definition, so the block still exits on the new
        // writer once the victim's store lands.
        let instructions = &mut function.blocks[0].instructions;
        instructions.insert(
            0,
            admission::instruction(
                SelectedInstructionId(88),
                SelectedInstructionKind::Store64 {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                store,
                &[VirtualRegisterId(0)],
            ),
        );
        instructions.insert(
            1,
            admission::instruction(
                SelectedInstructionId(90),
                SelectedInstructionKind::FrameAddress {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                address,
                &[VirtualRegisterId(51)],
            ),
        );
        instructions.insert(
            2,
            admission::instruction(
                SelectedInstructionId(91),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[VirtualRegisterId(51), VirtualRegisterId(52)],
            ),
        );
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(8), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    // Reuse declared nothing: the incumbent's entry stays the only slot.
    assert_eq!(
        function.local_storage_slots.as_slice(),
        [SelectedLocalStorageSlot {
            id: incumbent,
            byte_size: 8,
            alignment: 8,
        }]
    );
    // The victim's store after its definition names the shared slot, as do the
    // reload pair ahead of its join-block use.
    assert_eq!(
        function.blocks[0].instructions[4].kind,
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(incumbent),
            byte_offset: 0,
        }
    );
    let join = &function.blocks[2];
    let reload_pair = join.instructions.len() - 3;
    assert!(matches!(
        join.instructions[reload_pair].kind,
        SelectedInstructionKind::FrameAddress { slot, .. }
            if slot == FrameStorageSlotId::Local(incumbent)
    ));
    assert!(matches!(
        join.instructions[reload_pair + 1].kind,
        SelectedInstructionKind::Load64 { byte_offset: 0 }
    ));
    validate_runtime_spill(
        &source,
        0,
        VirtualRegisterId(8),
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A loop merge where the back edge delivers a different writer than the entry
/// arms keeps the incumbent's reload honest: the incumbent's own store sits on
/// the back edge behind the new victim's arrival store, so the merge's entry
/// set is `{EXISTING, NEW}` and sharing would let the incumbent's load read
/// the second victim's bytes on every entry path. The victim stays private.
#[test]
fn a_back_edge_writer_keeps_the_merge_load_private() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let keys = environment.selected_keys();
    let store = environment.constraint(keys.store64.unwrap()).unwrap();
    let address = environment.constraint(keys.frame_address.unwrap()).unwrap();
    let load = environment.constraint(keys.load64.unwrap()).unwrap();
    let mut source = cyclic_parameter_fixture(target, false);
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
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(52),
            scalar_type,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(91),
                source_value: ValueId::new(9).unwrap(),
            },
            definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
            entry_fixed_view: None,
        });
        let frame_slot = FrameStorageSlotId::Local(incumbent);
        // The incumbent reload sits at the head of the merge block — ahead of
        // the parameter's own uses — while its store rides the back edge
        // behind the arrival store the rewrite will emit.
        let merge = &mut function.blocks[2].instructions;
        merge.insert(
            0,
            admission::instruction(
                SelectedInstructionId(90),
                SelectedInstructionKind::FrameAddress {
                    slot: frame_slot,
                    byte_offset: 0,
                },
                address,
                &[VirtualRegisterId(51)],
            ),
        );
        merge.insert(
            1,
            admission::instruction(
                SelectedInstructionId(91),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[VirtualRegisterId(51), VirtualRegisterId(52)],
            ),
        );
        function.blocks[4].instructions.push(admission::instruction(
            SelectedInstructionId(502),
            SelectedInstructionKind::Store64 {
                slot: frame_slot,
                byte_offset: 0,
            },
            store,
            &[VirtualRegisterId(0)],
        ));
    }
    let identity = selected_instruction_plan_identity(source.transformed());
    source.receipt.source_selected = identity;
    source.receipt.transformed_selected = identity;
    let result =
        spill_selected_runtime_value(&source, 0, VirtualRegisterId(1), &environment, budget())
            .unwrap();
    let function = &result.transformed().functions[0];
    // The join's mixed writer set refused reuse: the victim's own slot is
    // appended behind the incumbent's declaration.
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
    // The incumbent's back-edge store and merge load kept their slot.
    assert!(matches!(
        function.blocks[4].instructions[2].kind,
        SelectedInstructionKind::Store64 { slot, .. }
            if slot == FrameStorageSlotId::Local(incumbent)
    ));
    assert!(matches!(
        function.blocks[2].instructions[0].kind,
        SelectedInstructionKind::FrameAddress { slot, .. }
            if slot == FrameStorageSlotId::Local(incumbent)
    ));
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
