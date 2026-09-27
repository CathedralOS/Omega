use super::{
    BETWEEN, KILLER, POINTER, SCRATCH, STORE, VALUE, access, budget, chained, fixture, instruction,
    landed_ids, mutated, pack_store, place, settlement, sink,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::store_motion::{
    StoreMutationMotionError, sink_selected_store_mutation, validate_store_mutation_motion,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use register_model::RegisterOperandAccess;
use selected_instructions::{
    LocalStorageSlotId, SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessRole, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, ValueId};
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
