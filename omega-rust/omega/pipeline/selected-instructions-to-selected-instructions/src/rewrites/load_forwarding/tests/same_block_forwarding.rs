use super::{
    BETWEEN, LOAD, MATERIALIZE_READ_INDEX, OUTPUT, POINTER, READ_INDEX, SCRATCH, SEQUENCE_INDEX,
    STORE, VALUE, access, budget, chained, define_index, define_index_as, fixture, forward,
    instruction, mutated, narrowed, place, sequence_pair, sequence_write,
};
use crate::ValidatedSelectedAnalysis;
use crate::{
    StoredLoadForwardingError, forward_selected_stored_load, validate_stored_load_forwarding,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedInstructionId, SelectedInstructionKind,
    SelectedLocalStorageSlot, SelectedMemoryAccess, SelectedMemoryAccessRole, VirtualRegisterId,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType, ValueId,
};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

#[test]
fn same_block_store_forwards_through_copy_and_drops_the_read_row() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let source_load = source.transformed().functions[0].blocks[0].instructions[3].clone();
        let result =
            forward_selected_stored_load(&source, 0, LOAD, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[0].instructions[3];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WritePlace]
        );
        assert_eq!(
            result.receipt().source_selected(),
            source.selected_identity()
        );
        assert_eq!(
            result.receipt().transformed_selected(),
            selected_instruction_plan_identity(result.transformed())
        );
        // The rewritten function detached from the source's shared storage.
        assert!(!std::ptr::eq(
            &source.transformed().functions[0],
            &result.transformed().functions[0]
        ));
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), detached)
            .unwrap();
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_forwarding() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = forward_selected_stored_load(&source, 0, LOAD, &environment, budget()).unwrap();
    for mutation in 0..9 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // Forwarded from the wrong register.
            0 => {
                function.blocks[0].instructions[3].operands[0].virtual_register = POINTER;
            }
            // Different result register.
            1 => {
                function.blocks[0].instructions[3].operands[1].virtual_register = SCRATCH;
            }
            // Kept the read row instead of dropping it.
            2 => {
                function.memory_accesses.push(access(
                    LOAD,
                    2,
                    place(),
                    0,
                    SelectedMemoryAccessRole::ReadPlace,
                ));
            }
            // Dropped the store's row as well.
            3 => {
                function.memory_accesses.remove(0);
            }
            // The surviving store must remain untouched.
            4 => {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the copy.
            5 => function.blocks[0].instructions[3].id = BETWEEN,
            // Fresh provenance must stay the read's.
            6 => {
                function.blocks[0].instructions[3]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            7 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // The read must not survive elsewhere in the block.
            8 => {
                function.blocks[0].instructions[3].kind =
                    SelectedInstructionKind::Load64 { byte_offset: 0 };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn overlapping_or_dynamic_writes_between_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Partial overwrite of the forwarded range cannot forward.
    let partial = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 4,
                ..access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace)
            },
        );
    });
    assert_eq!(
        forward(&partial, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A dynamic-extent byte-sequence write to the same place starting inside
    // the read's range still reaches its last byte.
    let dynamic = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                },
            ),
        );
    });
    assert_eq!(
        forward(&dynamic, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The same write one byte below the read's end still blocks, but a
    // dynamic-extent row reaches only upward from its fixed offset, so
    // starting at the read's end it is provably disjoint and walks past.
    let dynamic_edge = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    7,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        forward(&dynamic_edge, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    let dynamic_above = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    let forwarded_result = forward(&dynamic_above, &environment).unwrap();
    // The walk crossed the disjoint row to the store: the load forwarded and
    // its read row dropped while the dynamic row survives.
    assert_eq!(
        forwarded_result.transformed().functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::CopyI64
    );
    assert_eq!(
        forwarded_result.transformed().functions[0]
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_offset))
            .collect::<Vec<_>>(),
        vec![(STORE, 0), (BETWEEN, 8)]
    );
    validate_stored_load_forwarding(
        &dynamic_above,
        0,
        LOAD,
        &environment,
        budget(),
        forwarded_result.transformed().clone(),
    )
    .unwrap();
    // A span write past the read walks past the same way.
    let span_above = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    forward(&span_above, &environment).unwrap();
    // An operation-owned `Structural` slot write the contract does not charge
    // to the place's producer only stages bytes that name the place — a
    // call's staged view descriptor — so it holds none of the forwarded
    // bytes and walks past.
    let local = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    forward(&local, &environment).unwrap();
    // Materializing the place-backed local address lets later writes reach it.
    let address = mutated(target, |function, _| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::AddressLocal { slot },
            ),
        );
    });
    assert_eq!(
        forward(&address, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An unaccounted referent write (no row) rejects.
    let unaccounted = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
    });
    assert_eq!(
        forward(&unaccounted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn harmless_accesses_and_private_slots_still_forward() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A different place root cannot share writable storage with the forwarded
    // place under place exclusivity.
    let other_place = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                PlaceId::new(2).unwrap(),
                0,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    forward(&other_place, &environment).unwrap();
    // A disjoint range of the same place cannot touch the forwarded bytes.
    let disjoint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 16,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                16,
                SelectedMemoryAccessRole::WritePlace,
            ),
        );
    });
    forward(&disjoint, &environment).unwrap();
    // A re-read of the forwarded range cannot clobber it.
    let reread = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 0, SelectedMemoryAccessRole::ReadPlace),
        );
    });
    forward(&reread, &environment).unwrap();
    // Private spill-slot traffic carries no row and cannot alias a place.
    let spill = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(9),
                }),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    forward(&spill, &environment).unwrap();
}

/// A write into the forwarded place's own local storage sources the forward
/// exactly like a referent-pointer store: a parameter home or
/// block-parameter slot is the place's storage under the same byte
/// coordinates. The covering `Store64` names the slot directly; a place
/// `Store` through the slot's materialized address carries the same
/// `WriteLocal` row.
#[test]
fn place_storage_local_writes_forward() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // A direct `Store64` into the place's parameter home forwards its
        // single use operand.
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        let direct = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[1] = instruction(
                STORE,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[VALUE],
            );
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let source_load = direct.transformed().functions[0].blocks[0].instructions[3].clone();
        let result = forward(&direct, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[0].instructions[3];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        // The roster drops the read row and keeps the `WriteLocal` row.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WriteLocal { slot }]
        );
        validate_stored_load_forwarding(
            &direct,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A block-parameter slot is the place's storage the same way.
        let block_slot = LocalStorageSlotId::StructuralBlockParameter {
            block: semantic_vocabulary::BlockId::new(2).unwrap(),
            place: place(),
        };
        let parameter = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: block_slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[1] = instruction(
                STORE,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(block_slot),
                    byte_offset: 0,
                },
                store64,
                &[VALUE],
            );
            function.memory_accesses[0].role =
                SelectedMemoryAccessRole::WriteLocal { slot: block_slot };
        });
        let result = forward(&parameter, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0].blocks[0].instructions[3].kind,
            SelectedInstructionKind::CopyI64
        );
        // A place `Store` through the parameter slot's materialized address
        // carries the same `WriteLocal` row and sources the same register.
        let addressed = mutated(target, |function, _| {
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result = forward(&addressed, &environment).unwrap();
        let rewritten = &result.transformed().functions[0].blocks[0].instructions[3];
        assert_eq!(rewritten.kind, SelectedInstructionKind::CopyI64);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        validate_stored_load_forwarding(
            &addressed,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A `WriteLocal` row decides by range intersection like a `WritePlace` row:
/// a disjoint local write walks past whether its slot is the place's storage
/// or only stages bytes naming the place, and a `WriteLocal` row on another
/// place's slot is a different place root entirely.
#[test]
fn local_slot_writes_walk_past_disjoint_ranges() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A disjoint `Store64` into the place's own parameter slot cannot touch
    // the forwarded bytes: the slot is the place's storage and the row names
    // bytes outside the read's range.
    let disjoint = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 8,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                8,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    forward(&disjoint, &environment).unwrap();
    // The same disjoint write on a `Structural` staging slot stays harmless:
    // its bytes merely name the place and still sit outside the read's range.
    let disjoint_staging = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 8,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                8,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    forward(&disjoint_staging, &environment).unwrap();
    // A `WriteLocal` row on another place's parameter slot is a different
    // place root entirely.
    let other_place = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(2).unwrap(),
        };
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                PlaceId::new(2).unwrap(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    forward(&other_place, &environment).unwrap();
}

/// The local-slot writer must be the read's exact writer: a shifted
/// `Store64`, a `WriteLocal` row disagreeing with the instruction's slot or
/// route, a `Structural` staging slot, the wrong constraint, or a sub-word
/// read each leave the pair unproven.
#[test]
fn local_slot_sources_stay_exact() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `Store64` overlapping the read's tail is a shifted writer, not the
    // exact one: the head bytes still come from elsewhere.
    let shifted = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 4,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].byte_offset = 4;
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        forward(&shifted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The sourcing `Store64` must name the same slot its row claims.
    let wrong_slot = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let claimed = LocalStorageSlotId::StructuralParameter { place: place() };
        let written = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(2).unwrap(),
        };
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(written),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: claimed };
    });
    assert_eq!(
        forward(&wrong_slot, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A `WritePlace` row on the direct slot store is not the role the
    // instruction's route produces.
    let wrong_role = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    assert_eq!(
        forward(&wrong_role, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A `Store64` on the wrong constraint is not the target's slot store.
    let wrong_constraint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        forward(&wrong_constraint, &environment).unwrap_err(),
        StoredLoadForwardingError::ConstraintMismatch
    );
    // An operation-owned `Structural` slot the contract does not charge to
    // the place's producer stages bytes that merely name the place, so the
    // `Store64` into it walks past — and with no real writer upstream the
    // read is left unproven.
    let staging = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        forward(&staging, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A place `Store` through a staging slot's materialized address carries
    // the same non-storage `WriteLocal` row and cannot source either.
    let staged_store = mutated(target, |function, _| {
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal {
            slot: LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: place(),
            },
        };
    });
    assert_eq!(
        forward(&staged_store, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // The eight-byte slot store cannot produce a sub-word read's bytes.
    let subword = {
        let mut source = narrowed(target, 4);
        let inner = std::sync::Arc::make_mut(&mut source.transformed);
        let function = &mut inner.functions[0];
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        let identity = selected_instruction_plan_identity(&source.transformed);
        source.receipt.source_selected = identity;
        source.receipt.transformed_selected = identity;
        source
    };
    assert_eq!(
        forward(&subword, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

/// Replay of a local-slot-sourced forwarding holds the same exact contract:
/// the `WriteLocal` row kept, the load rewritten to the slot store's use
/// operand, and the surviving `Store64` untouched.
#[test]
fn local_source_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let source = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = forward(&source, &environment).unwrap();
    for mutation in 0..4 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // Forwarded from the wrong register.
            0 => {
                function.blocks[0].instructions[3].operands[0].virtual_register = POINTER;
            }
            // The sourcing row's `WriteLocal` identity must survive intact.
            1 => {
                function.memory_accesses[0].role = SelectedMemoryAccessRole::WritePlace;
            }
            // Dropped the writer's row as well as the read's.
            2 => {
                function.memory_accesses.remove(0);
            }
            // The surviving `Store64` must remain the same slot store.
            3 => {
                function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                        register: VirtualRegisterId(9),
                    }),
                    byte_offset: 0,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn sub_width_loads_forward_through_exact_width_stores() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        for (byte_size, expected) in [
            (1u8, SelectedInstructionKind::ZeroExtendU8),
            (2, SelectedInstructionKind::ZeroExtendU16),
            (4, SelectedInstructionKind::ZeroExtendU32),
        ] {
            let source = narrowed(target, byte_size);
            let source_load = source.transformed().functions[0].blocks[0].instructions[3].clone();
            let result = forward(&source, &environment).unwrap();
            let function = &result.transformed().functions[0];
            let rewritten = &function.blocks[0].instructions[3];
            assert_eq!(rewritten.id, LOAD);
            assert_eq!(rewritten.kind, expected);
            assert_eq!(rewritten.constraint, environment.selected_keys().copy_i64);
            assert_eq!(rewritten.operands.len(), 2);
            assert_eq!(rewritten.operands[0].virtual_register, VALUE);
            assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
            assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
            assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
            assert_eq!(rewritten.provenance, source_load.provenance);
            assert_eq!(
                function
                    .memory_accesses
                    .iter()
                    .map(|access| (access.role, access.byte_count))
                    .collect::<Vec<_>>(),
                vec![(SelectedMemoryAccessRole::WritePlace, u32::from(byte_size))]
            );
            // Replay restores the complete source by content.
            validate_stored_load_forwarding(
                &source,
                0,
                LOAD,
                &environment,
                budget(),
                result.transformed().clone(),
            )
            .unwrap();
        }
    }
}

#[test]
fn sub_width_loads_reject_wider_shifted_or_mismatched_sources() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A wider store keeps bytes the one-byte read cannot name portably: byte
    // order is the target's, so only an exact-width writer forwards.
    let wider = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&wider, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A byte read inside a wider store's range is not the forwarded value.
    let shifted = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 4 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[1].byte_offset = 4;
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&shifted, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A roster width disagreeing with the load kind is not the read's
    // identity; the pair is unsupported rather than a bad forward.
    let mismatched = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
    });
    assert_eq!(
        forward(&mismatched, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A store row claiming one byte while the instruction writes eight leaves
    // the read's neighbors unaccounted.
    let narrower_row = mutated(target, |function, environment| {
        let keys = environment.selected_keys();
        let load = environment.constraint(keys.load8.unwrap()).unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load,
            &[POINTER, OUTPUT],
        );
        function.memory_accesses[0].byte_count = 1;
        function.memory_accesses[1].byte_count = 1;
    });
    assert_eq!(
        forward(&narrower_row, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

#[test]
fn sub_width_replay_rejects_a_full_width_copy() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = narrowed(target, 1);
    let result = forward(&source, &environment).unwrap();
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[0].instructions[3].kind = SelectedInstructionKind::CopyI64;
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), proposed)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
}

#[test]
fn calls_hosted_effects_and_use_violations_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let call = mutated(target, |function, environment| {
        let call = environment
            .constraint(environment.selected_keys().call_unit[0])
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CallUnit {
                callee: MachineId::new(2).unwrap(),
            },
            call,
            &[],
        );
    });
    assert_eq!(
        forward(&call, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    // Redefining the carried value between store and load must not forward.
    let redefined = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, VALUE],
        );
    });
    assert_eq!(
        forward(&redefined, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot forward.
    assert_eq!(
        forward_selected_stored_load(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        StoredLoadForwardingError::SourceMismatch
    );
    // A non-load instruction id cannot forward.
    assert_eq!(
        forward_selected_stored_load(&source, 0, STORE, &environment, budget()).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        forward(&source, &foreign).unwrap_err(),
        StoredLoadForwardingError::SourceMismatch
    );
    // Without a preceding same-range store there is nothing to forward.
    let storeless = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(1);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        forward(&storeless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // The load's read row is required; a private reload has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        forward(&rowless, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedInstruction
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        forward_selected_stored_load(&source, 0, LOAD, &environment, tiny).unwrap_err(),
        StoredLoadForwardingError::WorkBudgetExceeded
    );
}

/// The measured work is the block scan, the walked interval plus crossed
/// edges, and the roster rows: five block slots, one interval step back to
/// the store, and two roster rows measure eight steps for the same-block
/// window; the crossed-edge window scans six slots, measures the successor's
/// head, the store's tail, and the crossed edge — two interval steps — plus
/// the same two rows, ten steps. Each exact boundary admits the forwarding
/// and replays it; one step below rejects the proposal and rejects even the
/// exact correct result under a starved replay budget.
#[test]
fn validation_budget_covers_the_walk() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (source, exact_steps) in [(fixture(target), 8u64), (chained(target), 10u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = forward_selected_stored_load(&source, 0, LOAD, &environment, exact).unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            forward_selected_stored_load(&source, 0, LOAD, &environment, starved).unwrap_err(),
            StoredLoadForwardingError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_stored_load_forwarding(
                &source,
                0,
                LOAD,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            StoredLoadForwardingError::WorkBudgetExceeded
        );
    }
}

/// The indexed byte load reads the byte at `base + index`; the matching
/// byte-sequence `Store { 0, 1 }` is its byte-exact writer, so the load
/// forwards to `ZeroExtendU8` of the stored register and drops only the
/// read row.
#[test]
fn byte_sequence_load_forwards_from_the_matching_sequence_store() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = mutated(target, |function, environment| {
            sequence_pair(function, environment, 0, 5);
        });
        let environment = baseline_target_register_environment(target).unwrap();
        let source_load = source.transformed().functions[0].blocks[0].instructions[3].clone();
        let result = forward(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        let rewritten = &function.blocks[0].instructions[3];
        assert_eq!(rewritten.id, LOAD);
        assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
        assert_eq!(rewritten.operands.len(), 2);
        assert_eq!(rewritten.operands[0].virtual_register, VALUE);
        assert_eq!(rewritten.operands[0].access, RegisterOperandAccess::Use);
        assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
        assert_eq!(rewritten.operands[1].access, RegisterOperandAccess::Def);
        assert_eq!(rewritten.provenance, source_load.provenance);
        // Only the read row drops; the covering sequence row survives.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| access.role)
                .collect::<Vec<_>>(),
            vec![SelectedMemoryAccessRole::WriteByteSequence {
                index: ValueId::new(5).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(7).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32]
                ),
            }]
        );
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), detached)
            .unwrap();
    }
}

/// The dynamic read's forwarding source stays byte-exact: same payload
/// base, same index value, one written byte. Every other writer — another
/// index, another base, an exact or span write, a slot store, or a second
/// roster row — may land on a different byte, so it rejects as the source
/// while still counting as interference on the walk.
#[test]
fn byte_sequence_forwarding_stays_byte_exact() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A sequence write at another index may land on a different byte.
    let other_index = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        let SelectedMemoryAccessRole::WriteByteSequence { index, .. } =
            &mut function.memory_accesses[0].role
        else {
            unreachable!()
        };
        *index = ValueId::new(6).unwrap();
    });
    assert_eq!(
        forward(&other_index, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A different payload base never spells the read's byte.
    let other_base = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses[0].byte_offset = 8;
    });
    assert_eq!(
        forward(&other_base, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An exact one-byte write cannot contain the runtime-placed read byte.
    let exact = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WritePlace;
    });
    assert_eq!(
        forward(&exact, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A span write's reach cannot be proven to land on the read byte.
    let span = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        let write = &mut function.memory_accesses[0];
        write.byte_count = 0;
        write.role = SelectedMemoryAccessRole::WriteByteSpan {
            length: ValueId::new(7).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
    });
    assert_eq!(
        forward(&span, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An encoded `Store` other than `{ 0, 1 }` never took the sequence
    // route, whatever its row claims.
    let widened = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        };
    });
    assert_eq!(
        forward(&widened, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The direct slot store names a slot, so it never carries the sequence
    // route either.
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let slot_store = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        });
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        forward(&slot_store, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A second roster row on the writer leaves its identity ambiguous.
    let two_rows = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses.insert(
            1,
            access(STORE, 3, place(), 0, SelectedMemoryAccessRole::WritePlace),
        );
    });
    assert_eq!(
        forward(&two_rows, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // The indexed load carrying an exact-range read row is not the sequence
    // read, and a fixed-offset byte load carrying the sequence row is not
    // either — each pair's kind and role must agree.
    let exact_read = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses[1].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        forward(&exact_read, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    let fixed_load = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        let load8 = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            load8,
            &[POINTER, OUTPUT],
        );
    });
    assert_eq!(
        forward(&fixed_load, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
    // A read row wider than the single indexed byte is not the sequence
    // read either.
    let wide_read = mutated(target, |function, environment| {
        sequence_pair(function, environment, 0, 5);
        function.memory_accesses[1].byte_count = 8;
    });
    assert_eq!(
        forward(&wide_read, &environment).unwrap_err(),
        StoredLoadForwardingError::UnsupportedPair
    );
}

/// The dynamic read extent mirrors the byte-sequence dead store's
/// interference directions: an exact or local row still reaches the read
/// byte once its own extent ends past the payload base — ending at or
/// below it is the only provable disjointness — and a dynamic-extent row
/// on the place always meets it.
#[test]
fn byte_sequence_walk_obeys_the_dynamic_extent_rule() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // An exact write ending at the payload base cannot touch the runtime
    // byte, so the walk crosses it to the real source.
    let below = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 0, SelectedMemoryAccessRole::WritePlace),
        );
    });
    let forwarded_result = forward(&below, &environment).unwrap();
    assert_eq!(
        forwarded_result.transformed().functions[0].blocks[0].instructions[3].kind,
        SelectedInstructionKind::ZeroExtendU8
    );
    validate_stored_load_forwarding(
        &below,
        0,
        LOAD,
        &environment,
        budget(),
        forwarded_result.transformed().clone(),
    )
    .unwrap();
    // Ending one byte past the base leaves the read byte reachable.
    let covering = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 8,
                ..access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace)
            },
        );
    });
    assert_eq!(
        forward(&covering, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A disjoint local write on the place's own storage ends at the base
    // the same way and walks past.
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let local_below = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    forward(&local_below, &environment).unwrap();
    // A dynamic-extent row on the place always meets the dynamic read —
    // here a sequence write at the same base but another index.
    let sequence_between = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 1,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(6).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        forward(&sequence_between, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A sequence write on another place never reaches this root.
    let other_place = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 1,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    BETWEEN,
                    3,
                    PlaceId::new(2).unwrap(),
                    8,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(5).unwrap(),
                        value: ValueId::new(6).unwrap(),
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    forward(&other_place, &environment).unwrap();
    // Materializing the place's own storage address still blocks.
    let address = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                8,
                SelectedMemoryAccessRole::AddressLocal { slot },
            ),
        );
    });
    assert_eq!(
        forward(&address, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

/// A byte-sequence row whose `index` resolves through the same carrier
/// audit the dead-store covering routes run touches exactly one byte —
/// `byte_offset + index` — wherever its payload base sits. The indexed
/// load's own index decides the same way: resolving it collapses the read
/// extent to that one fixed byte before the walk, so an exact one-byte
/// `Store` at the resolved position or a sequence write whose resolved
/// index lands on it each sources the forward, while a resolved write
/// landing anywhere else walks past to the real source.
#[test]
fn constant_index_read_collapses_to_a_fixed_byte() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The read's index materializes to 3 against payload base 8: the read
    // byte is fixed at 11, so the exact one-byte `Store` there — never a
    // dynamic read's writer — sources the forward.
    let exact = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
    });
    let result = forward(&exact, &environment).unwrap();
    let function = &result.transformed().functions[0];
    let rewritten = function.blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    assert_eq!(rewritten.operands[1].virtual_register, OUTPUT);
    // Only the read row drops; the covering write row survives.
    assert_eq!(
        function
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_offset, access.role))
            .collect::<Vec<_>>(),
        vec![(STORE, 11, SelectedMemoryAccessRole::WritePlace)]
    );
    validate_stored_load_forwarding(
        &exact,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A sequence write whose own index resolves lands on one byte; landing
    // on the read byte — payload base 4 plus index 7 — sources the forward
    // even though neither payload base nor index value matches the read's.
    let sequence = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses[0].byte_offset = 4;
        let SelectedMemoryAccessRole::WriteByteSequence { index, .. } =
            &mut function.memory_accesses[0].role
        else {
            unreachable!()
        };
        *index = ValueId::new(9).unwrap();
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    let result = forward(&sequence, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &sequence,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The same landing decides a fixed-offset byte load's forward: a
    // resolved sequence write landing on its byte sources it.
    let fixed_load = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        let load8 = environment
            .constraint(environment.selected_keys().load8.unwrap())
            .unwrap();
        let mut rewritten = instruction(
            LOAD,
            SelectedInstructionKind::Load8 { byte_offset: 11 },
            load8,
            &[POINTER, OUTPUT],
        );
        rewritten.provenance.operations = vec![OperationId::new(2).unwrap()];
        rewritten.provenance.values = vec![ValueId::new(4).unwrap()];
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == LOAD)
            {
                block.instructions[position] = rewritten.clone();
            }
        }
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(LOAD, 2, place(), 11, SelectedMemoryAccessRole::ReadPlace)
        };
        function.memory_accesses[0].byte_offset = 4;
        let SelectedMemoryAccessRole::WriteByteSequence { index, .. } =
            &mut function.memory_accesses[0].role
        else {
            unreachable!()
        };
        *index = ValueId::new(9).unwrap();
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    let result = forward(&fixed_load, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &fixed_load,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // An intervening sequence write whose resolved index lands off the read
    // byte — base 4 plus index 8 lands on 12 — walks past to the source.
    let walks_past = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
    });
    let result = forward(&walks_past, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &walks_past,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Landing on the read byte instead makes the intervening write the last
    // writer: it sources the forward with its own stored register.
    let lands_on = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    let result = forward(&lands_on, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.operands[0].virtual_register, SCRATCH);
    validate_stored_load_forwarding(
        &lands_on,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A wider exact write containing the read byte still cannot produce it
    // from one register — covering is not sourcing.
    let wider = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == BETWEEN)
            {
                block.instructions[position] = instruction(
                    BETWEEN,
                    SelectedInstructionKind::Store {
                        byte_offset: 10,
                        byte_size: 4,
                    },
                    store,
                    &[POINTER, SCRATCH],
                );
            }
        }
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 4,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    10,
                    SelectedMemoryAccessRole::WritePlace,
                )
            },
        );
    });
    assert_eq!(
        forward(&wider, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // An unresolved sequence write whose payload base sits at or below the
    // fixed read byte can still land on it — it interferes and cannot be
    // the byte-exact source.
    let runtime_writer = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 8, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 8, 9, SCRATCH);
    });
    assert_eq!(
        forward(&runtime_writer, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

/// A read index staying runtime keeps the dynamic extent, but a sequence
/// write whose own `index` resolves to a byte below the payload base is
/// provably disjoint from every byte the read can touch — it walks past to
/// the real source. A resolved landing at or past the base still meets the
/// read and, not being the byte-exact source, rejects.
#[test]
fn constant_index_writer_against_a_dynamic_read() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The read byte sits anywhere at or past base 8; the intervening
    // sequence write's resolved index lands it on byte 6 — provably below
    // the read's reach — so the walk continues to the matching sequence
    // store and forwards its register.
    let below = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    let result = forward(&below, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.kind, SelectedInstructionKind::ZeroExtendU8);
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &below,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Landing at the base — byte 8 — still meets the read's reach, and a
    // landing there cannot source a runtime-placed read byte.
    let at_base = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            4,
        );
    });
    assert_eq!(
        forward(&at_base, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // Distinct index values still spell the read byte when both resolve to
    // constants whose `byte_offset + index` sums agree — here the read's
    // own index resolves to a position no u32 names, so the extent stays
    // dynamic and the write's equal sum sources it.
    let equal_sums = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            u64::from(u32::MAX),
        );
        function.memory_accesses[0].byte_offset = 4;
        let SelectedMemoryAccessRole::WriteByteSequence { index, .. } =
            &mut function.memory_accesses[0].role
        else {
            unreachable!()
        };
        *index = ValueId::new(9).unwrap();
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            u64::from(u32::MAX) + 4,
        );
    });
    let result = forward(&equal_sums, &environment).unwrap();
    let rewritten = result.transformed().functions[0].blocks[0]
        .instructions
        .iter()
        .find(|instruction| instruction.id == LOAD)
        .unwrap();
    assert_eq!(rewritten.operands[0].virtual_register, VALUE);
    validate_stored_load_forwarding(
        &equal_sums,
        0,
        LOAD,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Distinct index values whose resolved sums disagree land on different
    // bytes — the write never spells the read's byte.
    let unequal_sums = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            u64::from(u32::MAX),
        );
        function.memory_accesses[0].byte_offset = 4;
        let SelectedMemoryAccessRole::WriteByteSequence { index, .. } =
            &mut function.memory_accesses[0].role
        else {
            unreachable!()
        };
        *index = ValueId::new(9).unwrap();
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            u64::from(u32::MAX) + 5,
        );
    });
    assert_eq!(
        forward(&unequal_sums, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
    // A resolved index still needs the clean carrier audit: two
    // `InstructionResult` registers claiming the index value leave it
    // ambiguous, so the write's reach stays runtime and — payload base at
    // or below the read's — it still interferes.
    let ambiguous = mutated(target, |function, environment| {
        sequence_pair(function, environment, 8, 5);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_READ_INDEX,
            READ_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        for block in &mut function.blocks {
            if let Some(position) = block
                .instructions
                .iter()
                .position(|instruction| instruction.id == STORE)
            {
                block.instructions[position] = instruction(
                    STORE,
                    SelectedInstructionKind::Store {
                        byte_offset: 11,
                        byte_size: 1,
                    },
                    store,
                    &[POINTER, VALUE],
                );
            }
        }
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 11, SelectedMemoryAccessRole::WritePlace)
        };
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_write(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_index(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
        function
            .virtual_registers
            .push(selected_instructions::VirtualRegister {
                id: VirtualRegisterId(20),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
                class: function.virtual_registers[0].class,
                origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(9),
                    source_value: ValueId::new(9).unwrap(),
                },
                definition_site: None,
                entry_fixed_view: None,
            });
    });
    assert_eq!(
        forward(&ambiguous, &environment).unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}
