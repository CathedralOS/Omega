use super::{
    BETWEEN, KILLER, MATERIALIZE_INDEX, MATERIALIZE_MOVED_INDEX, MOVED_SEQUENCE_INDEX,
    PACKED_SCRATCH, POINTER, SCRATCH, SEQUENCE_INDEX, STORE, VALUE, access, budget, chained,
    define_index, define_index_as, fixture, instruction, landed_ids, mutated, pack_store, place,
    sequence_row, sequence_store, settlement, sink,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::store_motion::{
    StoreMutationMotionError, sink_selected_store_mutation, validate_store_mutation_motion,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    LocalStorageSlotId, SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessRole, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, ScalarType, ValueId};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

#[test]
fn same_block_store_sinks_to_the_next_place_access() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
        );
        // The moved store keeps its identity, kind, operands, and provenance.
        assert_eq!(
            instructions[2].kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            }
        );
        assert_eq!(
            instructions[2],
            source.transformed().functions[0].blocks[0].instructions[1]
        );
        // Roster rows name instructions by identity: the roster is retained
        // unchanged even though the write sits later in the block.
        assert_eq!(
            function.memory_accesses,
            source.transformed().functions[0].memory_accesses
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
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), detached)
            .unwrap();
        // The store already sits at its latest position; replaying the motion
        // admits nothing.
        let replayed = crate::OwnedSelectedProgram::retain(&result);
        assert_eq!(
            sink_selected_store_mutation(&replayed, 0, STORE, &environment, budget()).unwrap_err(),
            StoreMutationMotionError::UnsupportedPair
        );
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_motion() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = sink_selected_store_mutation(&source, 0, STORE, &environment, budget()).unwrap();
    for mutation in 0..10 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The store must sit at the admitted position, not its old one.
            0 => {
                let moved = function.blocks[0].instructions.remove(2);
                function.blocks[0].instructions.insert(1, moved);
            }
            // The moved store must not disappear.
            1 => {
                function.blocks[0].instructions.remove(2);
            }
            // A different instruction must not move instead.
            2 => {
                let moved = function.blocks[0].instructions.remove(1);
                function.blocks[0].instructions.insert(2, moved);
            }
            // The moved write row must stay with the store.
            3 => {
                function
                    .memory_accesses
                    .retain(|access| access.instruction != STORE);
            }
            // A phantom row must not appear.
            4 => {
                function.memory_accesses.push(access(
                    BETWEEN,
                    3,
                    place(),
                    8,
                    SelectedMemoryAccessRole::ReadPlace,
                ));
            }
            // The surviving covering store must remain untouched.
            5 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the moved store.
            6 => function.blocks[0].instructions[2].id = BETWEEN,
            // Fresh provenance must stay the store's.
            7 => {
                function.blocks[0].instructions[2]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            8 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // A settlement row must not appear from nowhere.
            9 => function.boundary_settlements.push(settlement(3)),
            _ => unreachable!(),
        }
        assert!(
            validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn observing_accesses_land_the_store_just_before_them() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the moved range two positions later still observes the write:
    // the store lands immediately before it.
    let read = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Load64 { byte_offset: 0 },
                load,
                &[POINTER, SCRATCH],
            ),
        );
        function.memory_accesses.insert(
            2,
            access(
                SelectedInstructionId(6),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::ReadPlace,
            ),
        );
    });
    let result = sink(&read, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            BETWEEN,
            STORE,
            SelectedInstructionId(6),
            KILLER
        ]
    );
    // A disjoint range of the same place cannot observe the moved bytes, so
    // the store slides past it to the covering store.
    let disjoint = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 16 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 16, SelectedMemoryAccessRole::ReadPlace),
        );
    });
    let result = sink(&disjoint, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A read immediately after the store leaves no later position.
    let immediate = mutated(target, |function, environment| {
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
    assert_eq!(
        sink(&immediate, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A dynamic-extent write to the same place starting inside the moved
    // range still reaches its last byte, so it bounds the motion just as an
    // exact row does.
    let dynamic = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    0,
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
        sink(&dynamic, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // One byte below the moved range's end the same row still bounds the
    // motion, but a dynamic-extent row reaches only upward from its fixed
    // offset, so starting at the moved end it is provably disjoint and the
    // store slides past to the covering store.
    let dynamic_edge = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
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
        sink(&dynamic_edge, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    let dynamic_above = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
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
    let result = sink(&dynamic_above, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A span read past the moved range slides past the same way.
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
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: ValueId::new(7).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    let result = sink(&span_above, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A `WriteLocal` on a `Structural` slot the place's declaration does not
    // charge to that operation only stages bytes naming the place — its
    // writes are not the place's bytes at any offset, so the store slides
    // past it to the covering store.
    let staged = mutated(target, |function, environment| {
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
    let result = sink(&staged, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // The place's own storage — its parameter home — interferes by range: a
    // write into it at the moved offset must stay ordered after the store.
    let local = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
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
    assert_eq!(
        sink(&local, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A disjoint range of the place's own storage cannot observe the moved
    // bytes, so the store slides past it to the covering store.
    let disjoint_local = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
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
    let result = sink(&disjoint_local, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A materialized address of the place's own storage could reach the moved
    // bytes by a route the roster does not bound, so it bounds the window;
    // the same address on a staging slot reaches only the staged bytes.
    let materialized = |staging: bool| {
        mutated(target, move |function, environment| {
            let frame_address = environment
                .constraint(environment.selected_keys().frame_address.unwrap())
                .unwrap();
            let slot = if staging {
                LocalStorageSlotId::Structural {
                    operation: OperationId::new(9).unwrap(),
                    place: place(),
                }
            } else {
                LocalStorageSlotId::StructuralParameter { place: place() }
            };
            function
                .local_storage_slots
                .push(selected_instructions::SelectedLocalStorageSlot {
                    id: slot,
                    byte_size: 16,
                    alignment: 8,
                });
            function.blocks[0].instructions[2] = instruction(
                BETWEEN,
                SelectedInstructionKind::FrameAddress {
                    slot: selected_instructions::FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                frame_address,
                &[SCRATCH],
            );
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
        })
    };
    assert_eq!(
        sink(&materialized(false), &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    let result = sink(&materialized(true), &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
}

#[test]
fn carried_register_definitions_bound_the_motion() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Redefining the value register immediately after the store leaves no
    // later position.
    let value_def = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[SCRATCH, VALUE],
        );
    });
    assert_eq!(
        sink(&value_def, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The same redefinition one instruction later lets the store slide once.
    let one_step = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, SCRATCH],
        );
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::CopyI64,
                copy,
                &[SCRATCH, POINTER],
            ),
        );
    });
    let result = sink(&one_step, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            BETWEEN,
            STORE,
            SelectedInstructionId(6),
            KILLER
        ]
    );
}

#[test]
fn calls_unaccounted_and_settlement_positions_bound_the_motion() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A call observes through routes the roster cannot see.
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
        sink(&call, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // An unaccounted referent write (no row) bounds the window.
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
        sink(&unaccounted, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement immediately after the store would observe the place before
    // the moved write, so nothing later is reachable.
    let inside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(2));
    });
    assert_eq!(
        sink(&inside, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A settlement before the covering store keeps its position while the
    // store slides ahead of it.
    let boundary = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    let result = sink(&boundary, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![3]
    );
    // A settlement after the body also keeps its position.
    let tail = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(4));
    });
    let result = sink(&tail, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![4]
    );
}

#[test]
fn sub_width_stores_slide_within_the_same_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let narrow = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
            byte_offset: 4,
            byte_size: 4,
        };
        function.memory_accesses[0].byte_offset = 4;
        function.memory_accesses[0].byte_count = 4;
    });
    let result = sink(&narrow, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
}

/// A `StorePacked` sinks under the same window proof as the plain store,
/// but its early-clobber scratch `Def` and the target row's declared
/// clobbers move with it: a use of the scratch inside the window would read
/// the moved definition early, a second scratch write would be overtaken by
/// it, and on a flag-publishing target a flag writer or reader couples with
/// the moved clobber. Uses and definitions outside the window — before the
/// store's own position, or at and after the covering store that bounds it —
/// stay ordered correctly, and on a target whose packed row publishes no
/// condition state the same flag-pair window does not couple.
#[test]
fn packed_stores_sink_under_window_custody() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The seven-byte packed store slides past the copy to the covering
    // store, keeping its identity, operands, and the retained write row.
    let packed = mutated(target, |function, environment| {
        pack_store(function, environment);
    });
    let result = sink(&packed, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[2],
        packed.transformed().functions[0].blocks[0].instructions[1]
    );
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        packed.transformed().functions[0].memory_accesses
    );
    validate_store_mutation_motion(
        &packed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // Two runs are identical, and the landed plan is a terminal second
    // input: the packed store's next provable position is its own index.
    assert_eq!(sink(&packed, &environment).unwrap(), result);
    let replayed = crate::OwnedSelectedProgram::retain(&result);
    assert_eq!(
        sink_selected_store_mutation(&replayed, 0, STORE, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A scratch read inside the window observes the moved definition early.
    let scratch_read = mutated(target, |function, environment| {
        pack_store(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[PACKED_SCRATCH, SCRATCH],
        );
    });
    assert_eq!(
        sink(&scratch_read, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A second scratch definition inside the window is overtaken by the
    // moved one.
    let scratch_write = mutated(target, |function, environment| {
        pack_store(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, PACKED_SCRATCH],
        );
    });
    assert_eq!(
        sink(&scratch_write, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The same hazards one instruction later admit the partial landing.
    let partial = mutated(target, |function, environment| {
        pack_store(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            3,
            instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::CopyI64,
                copy,
                &[PACKED_SCRATCH, VALUE],
            ),
        );
    });
    let result = sink(&partial, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            BETWEEN,
            STORE,
            SelectedInstructionId(6),
            KILLER
        ]
    );
    // A scratch definition before the store's own position stays before it:
    // the window only orders the moved span.
    let before = mutated(target, |function, environment| {
        pack_store(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[0] = instruction(
            SelectedInstructionId(1),
            SelectedInstructionKind::CopyI64,
            copy,
            &[POINTER, PACKED_SCRATCH],
        );
    });
    let result = sink(&before, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A scratch use at or after the covering store — here the covering store
    // itself reads it — stays ordered after the moved definition.
    let after = mutated(target, |function, environment| {
        pack_store(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, PACKED_SCRATCH],
        );
    });
    let result = sink(&after, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // On x86 the packed row clobbers the flag unit: a flag writer inside the
    // window would be overtaken by the moved clobber, and a flag reader
    // would observe the moved flags — each bounds the motion.
    let flag_writer = mutated(target, |function, environment| {
        pack_store(function, environment);
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CompareI64,
            compare,
            &[POINTER, VALUE],
        );
    });
    assert_eq!(
        sink(&flag_writer, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    let flag_reader = mutated(target, |function, environment| {
        pack_store(function, environment);
        let boolean = environment
            .constraint(environment.selected_keys().materialize_boolean)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::MaterializeBooleanEqual,
            boolean,
            &[SCRATCH],
        );
    });
    assert_eq!(
        sink(&flag_reader, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The same flag-pair window does not couple on a target whose packed row
    // publishes no condition state: the store slides to the covering store.
    let flag_free_target = NativeTarget::linux_arm64();
    let flag_free = mutated(flag_free_target, |function, environment| {
        pack_store(function, environment);
        let compare = environment
            .constraint(environment.selected_keys().compare_i64)
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::CompareI64,
            compare,
            &[POINTER, VALUE],
        );
    });
    let flag_free_environment = baseline_target_register_environment(flag_free_target).unwrap();
    let result = sink(&flag_free, &flag_free_environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
}

/// The packed store must carry the target's declared `store_packed` row and
/// its exact `[use pointer, use packed value, def scratch]` surface, and its
/// single roster row must record the encoded offset and width on the route's
/// role: a different constraint key, a missing or misplaced scratch operand,
/// a non-defining scratch access, a second row, a disagreeing offset or
/// width, and a read role each reject.
#[test]
fn packed_store_surfaces_must_match_the_route() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The packed store must carry the target's declared packed row.
    let wrong_row = mutated(target, |function, environment| {
        pack_store(function, environment);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[1].constraint = copy.key;
    });
    assert_eq!(
        sink(&wrong_row, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    // The surface missing its scratch definition is not the declared row.
    let short = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.blocks[0].instructions[1].operands.pop();
    });
    assert_eq!(
        sink(&short, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    // A scratch operand out of its declared position mismatches the surface.
    let misplaced = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.blocks[0].instructions[1].operands[2].operand = 0;
    });
    assert_eq!(
        sink(&misplaced, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    // A scratch operand that does not define is not the packed row's shape.
    let non_defining = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.blocks[0].instructions[1].operands[2].access = RegisterOperandAccess::Use;
    });
    assert_eq!(
        sink(&non_defining, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    // A second row on the packed store is not the exact write surface.
    let extra = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.memory_accesses.insert(
            1,
            access(STORE, 9, place(), 0, SelectedMemoryAccessRole::WritePlace),
        );
    });
    assert_eq!(
        sink(&extra, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The row must record the packed encoding's own offset and width.
    let wide = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.memory_accesses[0].byte_count = 8;
    });
    assert_eq!(
        sink(&wide, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    let shifted = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.memory_accesses[0].byte_offset = 1;
    });
    assert_eq!(
        sink(&shifted, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A read row on the packed store is not the moved write.
    let read = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        sink(&read, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A proposal that drifts the packed store one slot early fails the
    // content replay even though the position is still legal-looking.
    let packed = mutated(target, |function, environment| {
        pack_store(function, environment);
    });
    let mut drifted = sink(&packed, &environment).unwrap().transformed().clone();
    let moved = drifted.functions[0].blocks[0].instructions.remove(2);
    drifted.functions[0].blocks[0].instructions.insert(1, moved);
    assert_eq!(
        validate_store_mutation_motion(&packed, 0, STORE, &environment, budget(), drifted)
            .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
}

/// The moved store can also reach the place's storage through its own local
/// slot: a `Store` through the parameter home's materialized address or a
/// `Store64` into that slot directly, each carrying the single `WriteLocal`
/// row on the place's storage. An operation-owned `Structural` home qualifies
/// only when the place's declaration names the operation as the result's
/// producer; without that charge the slot stages bytes that merely name the
/// place, and the store sinks as the staging slot's own subject instead.
/// Rejections keep their shape: a row disagreeing with the encoded slot, a
/// `WritePlace` row on the direct slot store, and a private spill slot that
/// carries no row at all.
#[test]
fn local_storage_stores_sink() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let parameter = LocalStorageSlotId::StructuralParameter { place: place() };
    let declare = |function: &mut selected_instructions::SelectedFunction, operation| {
        function.structural = Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: Vec::new().into(),
            parameters: Vec::new(),
            structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                id: place(),
                kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer: operation,
                    structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    };
    let push_slot = |function: &mut selected_instructions::SelectedFunction,
                     slot: LocalStorageSlotId| {
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
    };
    // The direct route: a `Store64` into the place's parameter home slides
    // past the copy to the covering store.
    let direct = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, parameter);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(parameter),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: parameter };
    });
    let result = sink(&direct, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    validate_store_mutation_motion(
        &direct,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A `Store` through the producer-declared `Structural` home's
    // materialized address carries the `WriteLocal` row and sinks the same
    // way.
    let producer = OperationId::new(9).unwrap();
    let home = LocalStorageSlotId::Structural {
        operation: producer,
        place: place(),
    };
    let addressed = mutated(target, |function, _| {
        declare(function, producer);
        push_slot(function, home);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: home };
    });
    let result = sink(&addressed, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // The same declaration without the producer charge leaves the slot a
    // staging slot: a `Store` carrying its `WriteLocal` row moves the slot's
    // own bytes, which the covering `WritePlace` on the place never reaches,
    // so the staging store slides past it to the block's end.
    let staged = mutated(target, |function, _| {
        push_slot(function, home);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: home };
    });
    let result = sink(&staged, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    validate_store_mutation_motion(
        &staged,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A `WriteLocal` row on a different place's parameter home is not the
    // moved place's storage either.
    let other = mutated(target, |function, _| {
        let slot = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(2).unwrap(),
        };
        push_slot(function, slot);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        sink(&other, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The direct `Store64`'s row must name the slot the instruction encodes:
    // a disagreement rejects rather than moving bytes under the wrong name.
    let mismatched = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let encoded = LocalStorageSlotId::StructuralParameter { place: place() };
        let named = LocalStorageSlotId::StructuralBlockParameter {
            block: semantic_vocabulary::BlockId::new(2).unwrap(),
            place: place(),
        };
        push_slot(function, encoded);
        push_slot(function, named);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(encoded),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: named };
    });
    assert_eq!(
        sink(&mismatched, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A `WritePlace` row on the direct `Store64` disagrees with the route the
    // instruction takes: the slot store does not write through a referent
    // pointer.
    let misroled = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, parameter);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(parameter),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    assert_eq!(
        sink(&misroled, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A `Store64` into a compiler-owned spill slot carries no roster row:
    // there is no place write to move.
    let spilled = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Spill {
            register: VirtualRegisterId(7),
        };
        push_slot(function, slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        sink(&spilled, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // The packed store takes the same `WriteLocal` route through the
    // producer-declared home's materialized address, its scratch custody
    // proven like the pointer route's.
    let packed_home = mutated(target, |function, environment| {
        declare(function, producer);
        push_slot(function, home);
        pack_store(function, environment);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: home };
    });
    let result = sink(&packed_home, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    validate_store_mutation_motion(
        &packed_home,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A staging slot's `WriteLocal` is the moved bytes' storage for the
    // packed route too: the scratch custody survives and the store slides
    // past the place's covering write to the block's end.
    let staged_packed = mutated(target, |function, environment| {
        push_slot(function, home);
        pack_store(function, environment);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot: home };
    });
    let result = sink(&staged_packed, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    validate_store_mutation_motion(
        &staged_packed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// A staging slot is itself a moved-store subject: a `Store64` into an
/// operation-owned `Structural` slot the place's declaration does not charge
/// to that operation — or a `Store`/`StorePacked` through the slot's
/// materialized address — writes bytes that only name the place under the
/// slot's own coordinates. The store sinks under the same walk, bounded by
/// the first row naming that very slot — a `WriteLocal` intersecting the
/// moved range or an `AddressLocal` exposing the staged bytes — while the
/// place's own accesses, other slots' rows, and a custody discard of the
/// staged place never reach the staged bytes at all.
#[test]
fn staging_slot_stores_sink_on_the_slot() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // No structural contract declares the operation the place's producer,
    // so the slot only stages bytes naming it.
    let slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(9).unwrap(),
        place: place(),
    };
    let push_slot = |function: &mut selected_instructions::SelectedFunction,
                     slot: LocalStorageSlotId| {
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
    };
    // The direct `Store64` into the staging slot sinks to just before the
    // covering `Store64` into the same slot.
    let direct = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = sink(&direct, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // The roster is retained: both rows stay `WriteLocal` on the slot.
    assert_eq!(
        result.transformed().functions[0]
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.role))
            .collect::<Vec<_>>(),
        vec![
            (STORE, SelectedMemoryAccessRole::WriteLocal { slot }),
            (KILLER, SelectedMemoryAccessRole::WriteLocal { slot })
        ]
    );
    validate_store_mutation_motion(
        &direct,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A `Store` through the staging slot's materialized address carries the
    // same `WriteLocal` row and sinks the same way.
    let addressed = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = sink(&addressed, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // Rows on the place's own storage never reach the staged bytes: a
    // `ReadPlace`, a `WritePlace`, and a `WriteLocal` on the parameter home
    // all walk past, so the staging store sinks to the block's end.
    let place_rows = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        push_slot(
            function,
            LocalStorageSlotId::StructuralParameter { place: place() },
        );
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 0, SelectedMemoryAccessRole::ReadPlace),
        );
        function.memory_accesses.push(access(
            BETWEEN,
            3,
            place(),
            0,
            SelectedMemoryAccessRole::WriteLocal {
                slot: LocalStorageSlotId::StructuralParameter { place: place() },
            },
        ));
    });
    let result = sink(&place_rows, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    // A write into a different staging slot of the same place moves other
    // bytes entirely and walks past; the covering same-slot write still
    // bounds the landing.
    let other_slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(10).unwrap(),
        place: place(),
    };
    let other_staging = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        push_slot(function, other_slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(other_slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot: other_slot },
            ),
        );
        function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = sink(&other_staging, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A disjoint range of the same staging slot cannot observe the moved
    // bytes, so the store slides past it to the covering write.
    let disjoint_slot = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 8,
            },
            store64,
            &[SCRATCH],
        );
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
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
        function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = sink(&disjoint_slot, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A materialized address of the staging slot could expose the staged
    // bytes by a route the roster does not bound, so it bounds the window.
    let materialized = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let frame_address = environment
            .constraint(environment.selected_keys().frame_address.unwrap())
            .unwrap();
        push_slot(function, slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            frame_address,
            &[SCRATCH],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        function.memory_accesses[1].role = SelectedMemoryAccessRole::AddressLocal { slot };
    });
    let result = sink(&materialized, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // A `WriteLocal` naming a different place than the slot stages is no
    // coherent staging row: the slot's bytes never name that place.
    let incoherent = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        push_slot(function, slot);
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
        function.memory_accesses[0] = access(
            STORE,
            1,
            PlaceId::new(2).unwrap(),
            0,
            SelectedMemoryAccessRole::WriteLocal { slot },
        );
    });
    assert_eq!(
        sink(&incoherent, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

#[test]
fn byte_sequence_stores_sink() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The byte-sequence store writes one byte at `offset + index` — a reach
    // unbounded upward from the payload base. The covering store's exact
    // write ends at that base, so the two never meet and the store slides
    // past every position to the block's end.
    let sunk = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
    });
    let result = sink(&sunk, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    assert_eq!(
        result.transformed().functions[0].memory_accesses,
        sunk.transformed().functions[0].memory_accesses
    );
    validate_store_mutation_motion(
        &sunk,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A covering write that reaches the payload base still can meet the
    // moved byte, so the store lands just before it.
    let reached = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses[1].byte_offset = 8;
    });
    let result = sink(&reached, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // The disjointness edge is the row's own extent: a read ending exactly
    // at the payload base slides past, while one ending a byte later can
    // observe the moved byte and stops the walk before any motion.
    let row_end_at_base = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
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
    let result = sink(&row_end_at_base, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    let row_end_past_base = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 1 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 1, SelectedMemoryAccessRole::ReadPlace),
        );
    });
    assert_eq!(
        sink(&row_end_past_base, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A dynamic-extent row on the moved place always meets a dynamic moved
    // extent — two unbounded-upward reaches can share a byte — even when the
    // row's fixed offset sits past the payload base.
    let dynamic_later = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    24,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(11).unwrap(),
                        value: ValueId::new(12).unwrap(),
                        length: ValueId::new(13).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([4; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        sink(&dynamic_later, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A dynamic read on the moved place meets the moved extent the same way.
    let dynamic_read = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: ValueId::new(13).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([4; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        sink(&dynamic_read, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // Dynamic rows on another place never meet the moved byte.
    let other_place = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    PlaceId::new(2).unwrap(),
                    0,
                    SelectedMemoryAccessRole::WriteByteSequence {
                        index: ValueId::new(11).unwrap(),
                        value: ValueId::new(12).unwrap(),
                        length: ValueId::new(13).unwrap(),
                        obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([4; 32]),
                    },
                )
            },
        );
    });
    let result = sink(&other_place, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    // A `WriteLocal` on the place's own storage decides by its exact extent:
    // ending at the payload base it is disjoint and slides past; reaching
    // past it, the store lands before the write.
    let parameter = LocalStorageSlotId::StructuralParameter { place: place() };
    let local_disjoint = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: parameter,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(parameter),
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
                SelectedMemoryAccessRole::WriteLocal { slot: parameter },
            ),
        );
    });
    let result = sink(&local_disjoint, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, KILLER, STORE]
    );
    let local_reaching = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: parameter,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(parameter),
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
                SelectedMemoryAccessRole::WriteLocal { slot: parameter },
            ),
        );
    });
    assert_eq!(
        sink(&local_reaching, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A materialized address of the place's own storage can reach the moved
    // byte by a route the roster does not bound, so it stops the walk.
    let materialized = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        let frame_address = environment
            .constraint(environment.selected_keys().frame_address.unwrap())
            .unwrap();
        function
            .local_storage_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: parameter,
                byte_size: 16,
                alignment: 8,
            });
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(parameter),
                byte_offset: 0,
            },
            frame_address,
            &[SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            access(
                BETWEEN,
                3,
                place(),
                0,
                SelectedMemoryAccessRole::AddressLocal { slot: parameter },
            ),
        );
    });
    assert_eq!(
        sink(&materialized, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

#[test]
fn byte_sequence_store_route_must_match() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The sequence route admits only the one-byte store through the computed
    // address: any other encoded offset or width disagrees with the row.
    for (byte_offset, byte_size) in [(1, 1), (0, 4)] {
        let mismatched = mutated(target, |function, environment| {
            sequence_store(function, environment, 8);
            function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
                byte_offset,
                byte_size,
            };
        });
        assert_eq!(
            sink(&mismatched, &environment).unwrap_err(),
            StoreMutationMotionError::UnsupportedPair
        );
    }
    // The row names exactly one written byte; a different count disagrees.
    let wrong_count = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses[0].byte_count = 2;
    });
    assert_eq!(
        sink(&wrong_count, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A second roster row on the moved store leaves its reach unaccounted.
    let second_row = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        function.memory_accesses.insert(
            1,
            access(STORE, 2, place(), 8, SelectedMemoryAccessRole::WritePlace),
        );
    });
    assert_eq!(
        sink(&second_row, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The packed and direct-slot routes never carry a sequence row: the
    // packed store's width is never one byte and the slot store writes a
    // named slot, not a computed view address.
    let packed = mutated(target, |function, environment| {
        pack_store(function, environment);
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            role: SelectedMemoryAccessRole::WriteByteSequence {
                index: ValueId::new(5).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(7).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32],
                ),
            },
            ..access(STORE, 1, place(), 0, SelectedMemoryAccessRole::WritePlace)
        };
    });
    assert_eq!(
        sink(&packed, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    let direct = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(
                    LocalStorageSlotId::StructuralParameter { place: place() },
                ),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    assert_eq!(
        sink(&direct, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The replay consumes the moved store's row as proposed: flipping it to
    // a plain place write no longer matches the admitted roster.
    let sunk = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
    });
    let result = sink(&sunk, &environment).unwrap();
    let mut proposed = result.transformed().clone();
    proposed.functions[0].memory_accesses[0].role = SelectedMemoryAccessRole::WritePlace;
    assert_eq!(
        validate_store_mutation_motion(&sunk, 0, STORE, &environment, budget(), proposed)
            .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot move.
    assert_eq!(
        sink_selected_store_mutation(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
    // A non-store instruction id cannot move.
    assert_eq!(
        sink_selected_store_mutation(&source, 0, BETWEEN, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        sink(&source, &foreign).unwrap_err(),
        StoreMutationMotionError::SourceMismatch
    );
    // The store's write row is required; a private store has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        sink(&rowless, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // A read row on the store instruction is not a write.
    let mislabeled = mutated(target, |function, _| {
        function.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        sink(&mislabeled, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A second row on the store is not the exact write surface.
    let extra = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            access(STORE, 9, place(), 0, SelectedMemoryAccessRole::WritePlace),
        );
    });
    assert_eq!(
        sink(&extra, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A row offset that does not match the instruction's encoding rejects.
    let shifted_row = mutated(target, |function, _| {
        function.memory_accesses[0].byte_offset = 8;
    });
    assert_eq!(
        sink(&shifted_row, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A store width outside the produced grammar is unsupported.
    let exotic = mutated(target, |function, _| {
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 3,
        };
        function.memory_accesses[0].byte_count = 3;
    });
    assert_eq!(
        sink(&exotic, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedInstruction
    );
    // A different constraint row does not give the plain [use, use] surface.
    let wrong_row = mutated(target, |function, environment| {
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions[1].constraint = copy.key;
    });
    assert_eq!(
        sink(&wrong_row, &environment).unwrap_err(),
        StoreMutationMotionError::ConstraintMismatch
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        sink_selected_store_mutation(&source, 0, STORE, &environment, tiny).unwrap_err(),
        StoreMutationMotionError::WorkBudgetExceeded
    );
}

/// Two runs over the identical source produce the identical validated result,
/// and the published plan is a legal second input: the sealed transformed
/// program already sits at the rule's fixed point, so the moved store's next
/// provable position is its own index and the covering store's is too.
#[test]
fn motion_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let first = sink(&fixture(target), &environment).unwrap();
    let second = sink(&fixture(target), &environment).unwrap();
    assert_eq!(first, second);
    // The validated output carries the sealed analysis boundary, so it is a
    // legal second input — not merely a reconstruction of one. Re-running on
    // it is terminal: the position after the moved store is the covering
    // store itself, so the walk lands back on the store's own index.
    assert_eq!(
        sink_selected_store_mutation(&first, 0, STORE, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // The covering store's next provable position is its own index as well:
    // a terminator without successors leaves no later position.
    assert_eq!(
        sink_selected_store_mutation(&first, 0, KILLER, &environment, budget()).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// The measured work is the block scan, the walked interval, and the roster
/// rows: five block slots, two interval steps through the covering store,
/// and two roster rows measure nine steps for the same-block window; the
/// crossed-edge window scans six slots, measures the crossed tail, the
/// crossed edge, and the covering block — four interval steps — plus the
/// same two rows, twelve steps. Each exact boundary admits the motion and
/// replays it; one step below rejects the proposal and rejects even the
/// exact correct result under a starved replay budget.
#[test]
fn validation_budget_covers_the_walk() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (source, exact_steps) in [(fixture(target), 9u64), (chained(target), 12u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = sink_selected_store_mutation(&source, 0, STORE, &environment, exact).unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            sink_selected_store_mutation(&source, 0, STORE, &environment, starved).unwrap_err(),
            StoreMutationMotionError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_store_mutation_motion(
                &source,
                0,
                STORE,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            StoreMutationMotionError::WorkBudgetExceeded
        );
    }
}

/// A byte-sequence row whose `index` resolves through the carrier audit —
/// sole `InstructionResult` carrier, clean `MaterializeI64` definition, no
/// edge-transport or case-payload redefinition — touches exactly the byte
/// `byte_offset + index` wherever its payload base sits, so a landing off
/// the moved extent walks past instead of bounding the window. The moved
/// byte is decided the same way: a moved index resolving to a materialized
/// constant collapses the extent to that one byte, so a resolved row lands
/// off it below or above, and an exact row that does not reach it slides
/// past like any disjoint row.
#[test]
fn constant_index_sequence_rows_landing_off_the_moved_extent_walk_past() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The intervening write's payload base sits inside the moved range
    // [0, 8), but its index materializes to 8, landing the byte at 12 —
    // provably off the moved bytes, so the store slides past it and its
    // materialize to the covering store.
    let off = mutated(target, |function, environment| {
        sequence_row(function, BETWEEN, 4, 9);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
    });
    let result = sink(&off, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_INDEX,
            BETWEEN,
            STORE,
            KILLER
        ]
    );
    validate_store_mutation_motion(
        &off,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A landing above the moved extent is off it the same way: payload base
    // 4 plus index 12 lands on byte 16.
    let above = mutated(target, |function, environment| {
        sequence_row(function, BETWEEN, 4, 9);
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            12,
        );
    });
    let result = sink(&above, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_INDEX,
            BETWEEN,
            STORE,
            KILLER
        ]
    );
    // A runtime index on the intervening row keeps its reach unbounded
    // upward, but a payload base at or past the moved end is still provably
    // disjoint — the same disjointness the unresolved rows already had.
    let runtime_row = mutated(target, |function, _| {
        sequence_row(function, BETWEEN, 8, 9);
    });
    let result = sink(&runtime_row, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![SelectedInstructionId(1), BETWEEN, STORE, KILLER]
    );
    // The moved store's own index resolves through the same audit: the
    // extent collapses to the one byte `8 + 3` = 11, so the covering
    // store's exact write at [0, 8) is disjoint from it and the store
    // sinks to the block's end.
    let collapsed = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
    });
    let result = sink(&collapsed, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_MOVED_INDEX,
            BETWEEN,
            KILLER,
            STORE
        ]
    );
    validate_store_mutation_motion(
        &collapsed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // The collapsed extent still decides by position: the same resolved
    // index against payload base 4 lands the moved byte at 6, inside the
    // covering store's [0, 8), so the store lands just before it.
    let collapsed_on = mutated(target, |function, environment| {
        sequence_store(function, environment, 4);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            2,
        );
    });
    let result = sink(&collapsed_on, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_MOVED_INDEX,
            BETWEEN,
            STORE,
            KILLER
        ]
    );
    // A collapsed moved extent is met by a resolved row only when it lands
    // on that byte: the intervening write resolves to `4 + 7` = 11, the
    // moved byte itself, so the stop is the store's own next position and
    // no motion admits.
    let row_on_collapsed = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        sequence_row(function, BETWEEN, 4, 9);
        define_index(
            function,
            environment,
            0,
            4,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    assert_eq!(
        sink(&row_on_collapsed, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// The resolved landing is only safe off the moved bytes: a write or read
/// landing on the moved extent still bounds the window, a landing at or
/// past an unresolved moved payload base may still meet the runtime-placed
/// byte, and an unresolved index leaves the row's reach unbounded upward
/// into a collapsed moved byte. An index whose carrier is not one clean
/// `MaterializeI64` — a copy's result, or two carriers claiming one value —
/// resolves nothing, so the moved extent stays dynamic.
#[test]
fn constant_index_sequence_rows_still_interfere_on_the_moved_extent() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // The intervening write resolves to `4 + 2` = 6 — inside the moved
    // range [0, 8) — so it bounds the window: the next position is already
    // the stop, leaving no later landing.
    let write_on = mutated(target, |function, environment| {
        sequence_row(function, BETWEEN, 4, 9);
        define_index(
            function,
            environment,
            0,
            1,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    assert_eq!(
        sink(&write_on, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A resolved read landing on the moved range observes the moved byte:
    // the same stop through the `ReadByteSequence` role.
    let read_on = mutated(target, |function, environment| {
        function.memory_accesses.push(SelectedMemoryAccess {
            byte_count: 1,
            ..access(
                BETWEEN,
                3,
                place(),
                4,
                SelectedMemoryAccessRole::ReadByteSequence {
                    index: ValueId::new(9).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                },
            )
        });
        define_index(
            function,
            environment,
            0,
            1,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    assert_eq!(
        sink(&read_on, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A moved index staying runtime leaves the moved byte anywhere at or
    // past the payload base: the intervening write landing on byte 16 may
    // be that byte, so it still interferes.
    let in_reach = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        sequence_row(function, BETWEEN, 16, 9);
        define_index(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            0,
        );
    });
    assert_eq!(
        sink(&in_reach, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // An unresolved index leaves the row's reach unbounded upward: a
    // payload base at or below the collapsed moved byte can still land on
    // it.
    let runtime_index = mutated(target, |function, environment| {
        sequence_store(function, environment, 8);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        sequence_row(function, BETWEEN, 4, 9);
    });
    assert_eq!(
        sink(&runtime_index, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
    // A moved index defined by a copy rather than a clean `MaterializeI64`
    // resolves nothing: the extent stays dynamic — unbounded upward from
    // the payload base 4 — so the covering store's [0, 8) still reaches it
    // and bounds the motion.
    let copied = mutated(target, |function, environment| {
        sequence_store(function, environment, 4);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function
            .virtual_registers
            .push(selected_instructions::VirtualRegister {
                id: MOVED_SEQUENCE_INDEX,
                scalar_type: ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
                class: copy.operands[0].class,
                origin: selected_instructions::VirtualRegisterOrigin::InstructionResult {
                    instruction: MATERIALIZE_MOVED_INDEX,
                    source_value: ValueId::new(5).unwrap(),
                },
                definition_site: None,
                entry_fixed_view: None,
            });
        function.blocks[0].instructions.insert(
            1,
            instruction(
                MATERIALIZE_MOVED_INDEX,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, MOVED_SEQUENCE_INDEX],
            ),
        );
    });
    let result = sink(&copied, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_MOVED_INDEX,
            BETWEEN,
            STORE,
            KILLER
        ]
    );
    // Two carriers claiming the moved index's value make the constant
    // ambiguous, so the extent stays dynamic the same way.
    let ambiguous = mutated(target, |function, environment| {
        sequence_store(function, environment, 4);
        define_index_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_MOVED_INDEX,
            MOVED_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_index(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
    });
    let result = sink(&ambiguous, &environment).unwrap();
    assert_eq!(
        landed_ids(&result),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_MOVED_INDEX,
            MATERIALIZE_INDEX,
            BETWEEN,
            STORE,
            KILLER
        ]
    );
}
