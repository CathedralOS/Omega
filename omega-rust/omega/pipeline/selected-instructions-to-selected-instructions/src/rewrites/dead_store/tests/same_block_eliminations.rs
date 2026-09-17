use super::{
    BETWEEN, KILLER, PACKED_SCRATCH, POINTER, SCRATCH, STORE, VALUE, access, budget, chained,
    eliminate, fixture, instruction, make_packed_dead, mutated, packed_dead, place, settlement,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::dead_store::{
    DeadStoreEliminationError, eliminate_selected_dead_store, validate_dead_store_elimination,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, PackedByteWidth, SelectedInstructionId,
    SelectedInstructionKind, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessRole, VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, ScalarType, ValueId};
use target::NativeTarget;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

#[test]
fn same_block_covering_store_eliminates_and_drops_the_write_row() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let source = fixture(target);
        let environment = baseline_target_register_environment(target).unwrap();
        let result =
            eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
        let function = &result.transformed().functions[0];
        let instructions = &function.blocks[0].instructions;
        assert_eq!(
            instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        // The covering store keeps its identity, operands, and provenance.
        assert_eq!(
            instructions[2].kind,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            }
        );
        assert_eq!(
            instructions[2].provenance,
            source.transformed().functions[0].blocks[0].instructions[3].provenance
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
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
        validate_dead_store_elimination(
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
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), detached)
            .unwrap();
    }
}

/// A write into the dead place's own local storage covers the dead range
/// exactly like a place store: a parameter home or block-parameter slot is
/// the place's storage under the same byte coordinates. The covering
/// `Store64` names the slot directly; a place `Store` through the slot's
/// materialized address carries the same `WriteLocal` row.
#[test]
fn place_storage_local_writes_cover() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // A direct `Store64` into the place's parameter home.
        let direct = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            let slot = LocalStorageSlotId::StructuralParameter { place: place() };
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result = eliminate(&direct, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                SelectedMemoryAccessRole::WriteLocal {
                    slot: LocalStorageSlotId::StructuralParameter { place: place() },
                }
            )]
        );
        validate_dead_store_elimination(
            &direct,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A block-parameter slot is the place's storage the same way.
        let parameter = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            let slot = LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(2).unwrap(),
                place: place(),
            };
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&parameter, &environment).unwrap();
        // A place `Store` through the parameter slot's materialized address
        // carries the same `WriteLocal` row and covers the same bytes.
        let addressed = mutated(target, |function, _| {
            let slot = LocalStorageSlotId::StructuralParameter { place: place() };
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&addressed, &environment).unwrap();
    }
}

/// A `WriteLocal` row decides by range intersection like a `WritePlace` row:
/// a disjoint local write walks past, and an intersecting one still has to
/// cover. Operation-owned `Structural` slots never cover — they can stage
/// bytes that merely name the place — and a slot or role that disagrees
/// with the covering instruction rejects.
#[test]
fn local_slot_covering_writes_stay_exact() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A disjoint write into the place's own parameter slot cannot touch the
    // dead bytes: the slot is the place's storage and the row names bytes
    // outside the dead range.
    let disjoint = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
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
    eliminate(&disjoint, &environment).unwrap();
    // The same disjoint write on a `Structural` slot stays harmless too:
    // even a staging slot's bytes outside the dead range cannot touch it.
    let disjoint_staging = mutated(target, |function, environment| {
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
    eliminate(&disjoint_staging, &environment).unwrap();
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
    eliminate(&other_place, &environment).unwrap();
    // A partial-range parameter-slot write leaves the head bytes live.
    let partial = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 4,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_offset: 4,
            ..access(
                KILLER,
                2,
                place(),
                4,
                SelectedMemoryAccessRole::WriteLocal { slot },
            )
        };
    });
    assert_eq!(
        eliminate(&partial, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The covering `Store64` must name the same slot its row claims.
    let wrong_slot = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let claimed = LocalStorageSlotId::StructuralParameter { place: place() };
        let written = LocalStorageSlotId::StructuralParameter {
            place: PlaceId::new(2).unwrap(),
        };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(written),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot: claimed };
    });
    assert_eq!(
        eliminate(&wrong_slot, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A `WritePlace` row on the direct slot store is not the role the
    // instruction's route produces.
    let wrong_role = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
    });
    assert_eq!(
        eliminate(&wrong_role, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A `Store64` on the wrong constraint is not the target's slot store.
    let wrong_constraint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        eliminate(&wrong_constraint, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // An operation-owned `Structural` slot can stage bytes that merely name
    // the place, so even a covering-range write into it cannot cover.
    let staging = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        eliminate(&staging, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// Replay of a local-slot covering elimination holds the same exact-removal
/// contract: the dead store retained, the dead write row kept, the covering
/// `WriteLocal` row flipped or dropped, or the covering instruction altered
/// each drift from the independently derived result.
#[test]
fn local_covering_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let source = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..5 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead store must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // Kept the dead write row instead of dropping it.
            1 => {
                function.memory_accesses.push(access(
                    STORE,
                    1,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WritePlace,
                ));
            }
            // The covering row's `WriteLocal` identity must survive intact.
            2 => {
                function.memory_accesses[0].role = SelectedMemoryAccessRole::WritePlace;
            }
            // Dropped the covering store's row as well.
            3 => {
                function.memory_accesses.clear();
            }
            // The surviving `Store64` must remain the same slot store.
            4 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                        register: VirtualRegisterId(9),
                    }),
                    byte_offset: 0,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn replay_rejects_anything_but_the_exact_elimination() {
    let target = NativeTarget::linux_x64();
    let source = fixture(target);
    let environment = baseline_target_register_environment(target).unwrap();
    let result = eliminate_selected_dead_store(&source, 0, STORE, &environment, budget()).unwrap();
    for mutation in 0..9 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead store must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // A different instruction must not disappear instead.
            1 => {
                function.blocks[0].instructions.remove(1);
            }
            // Kept the dead write row instead of dropping it.
            2 => {
                function.memory_accesses.push(access(
                    STORE,
                    1,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WritePlace,
                ));
            }
            // Dropped the covering store's row as well.
            3 => {
                function.memory_accesses.clear();
            }
            // The surviving store must remain untouched.
            4 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            // A different instruction id on the killer.
            5 => function.blocks[0].instructions[2].id = BETWEEN,
            // Fresh provenance must stay the killer's.
            6 => {
                function.blocks[0].instructions[2]
                    .provenance
                    .values
                    .push(ValueId::new(9).unwrap());
            }
            // An unrelated register must stay identical.
            7 => function.virtual_registers[1].scalar_type = ScalarType::Boolean,
            // A settlement row must not appear from nowhere.
            8 => function.boundary_settlements.push(settlement(3)),
            _ => unreachable!(),
        }
        assert!(
            validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn observing_or_partial_accesses_between_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the dead range observes the stored bytes.
    let read = mutated(target, |function, environment| {
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
        eliminate(&read, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dynamic-extent byte-sequence read of the same place cannot be proven
    // to stay out of the dead range.
    let dynamic_read = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::ReadByteSequence {
                        index: ValueId::new(5).unwrap(),
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
        eliminate(&dynamic_read, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A partial overwrite leaves the remaining dead bytes observable.
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
        eliminate(&partial, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dynamic-extent write to the same place cannot be proven to cover.
    let dynamic_write = mutated(target, |function, _| {
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
        eliminate(&dynamic_write, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A place-backed local slot write targets the dead place's storage.
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
    assert_eq!(
        eliminate(&local, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // Materializing the place-backed local address lets later accesses reach
    // it by a route the roster cannot prove disjoint.
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
        eliminate(&address, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
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
        eliminate(&unaccounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A covering-range store at a different offset leaves the dead bytes live.
    let elsewhere = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_offset = 8;
    });
    assert_eq!(
        eliminate(&elsewhere, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

#[test]
fn harmless_accesses_and_private_slots_still_eliminate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A different place root cannot share storage with the dead place under
    // place exclusivity.
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
    eliminate(&other_place, &environment).unwrap();
    // A disjoint range of the same place cannot touch the dead bytes.
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
    eliminate(&disjoint, &environment).unwrap();
    // Private spill-slot traffic carries no row and cannot alias a place.
    let spill = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Store64 {
                slot: selected_instructions::FrameStorageSlotId::Local(LocalStorageSlotId::Spill {
                    register: VirtualRegisterId(9),
                }),
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    eliminate(&spill, &environment).unwrap();
    // A private-slot reload without a row cannot observe referent storage.
    let reload = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[2] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        );
    });
    eliminate(&reload, &environment).unwrap();
}

#[test]
fn calls_hosted_effects_and_settlement_positions_reject() {
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
        eliminate(&call, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // A settlement positioned before an interval instruction could observe
    // the dead bytes at the boundary.
    let inside = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    assert_eq!(
        eliminate(&inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

#[test]
fn settlements_after_the_covering_store_shift_left() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let shifted = mutated(target, |function, _| {
        // Before the dead store: unchanged. After the covering store: shifts.
        function.boundary_settlements.push(settlement(1));
        function.boundary_settlements.push(settlement(4));
    });
    let result = eliminate(&shifted, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .boundary_settlements
            .iter()
            .map(|settlement| settlement.instruction_index)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
}

#[test]
fn admission_boundaries_and_budget_hold() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // A nonexistent instruction id cannot eliminate.
    assert_eq!(
        eliminate_selected_dead_store(
            &source,
            0,
            SelectedInstructionId(99),
            &environment,
            budget()
        )
        .unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // A non-store instruction id cannot eliminate.
    assert_eq!(
        eliminate_selected_dead_store(&source, 0, BETWEEN, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // The wrong target's environment is a different source.
    let foreign = baseline_target_register_environment(NativeTarget::linux_arm64()).unwrap();
    assert_eq!(
        eliminate(&source, &foreign).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
    // Without a later same-range store the bytes stay observable.
    let open = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        eliminate(&open, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The store's write row is required; a private store has nothing to prove.
    let rowless = mutated(target, |function, _| {
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        eliminate(&rowless, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // A read row on the store instruction is not a write.
    let mislabeled = mutated(target, |function, _| {
        function.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        eliminate(&mislabeled, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    let tiny = OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap();
    assert_eq!(
        eliminate_selected_dead_store(&source, 0, STORE, &environment, tiny).unwrap_err(),
        DeadStoreEliminationError::WorkBudgetExceeded
    );
}

/// A dead store narrower than eight bytes is eliminated by any later place
/// store whose `WritePlace` row covers its whole range: a same-width store at
/// the same offset, a wider store starting earlier, or a packed store.
#[test]
fn sub_width_dead_stores_and_covering_killers_eliminate() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A four-byte dead store killed by the same-width store at the same
    // offset: the narrowest exact pair.
    let exact = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(STORE, 1, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(KILLER, 2, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
    });
    let result = eliminate(&exact, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, KILLER]
    );
    assert_eq!(
        result.transformed().functions[0]
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_count))
            .collect::<Vec<_>>(),
        vec![(KILLER, 4)]
    );
    // A wider covering store starting before the dead range rewrites every
    // dead byte: dead `Store` of four bytes at offset 4, killer `Store` of
    // eight at offset 0.
    let wider = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(STORE, 1, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
    });
    eliminate(&wider, &environment).unwrap();
    // A packed store covers a shifted dead range: six bytes at offset 2
    // contain the dead four bytes at offset 4.
    let packed = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        let packed = environment
            .constraint(environment.selected_keys().store_packed.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 4,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(STORE, 1, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::StorePacked {
                byte_offset: 2,
                width: PackedByteWidth::Six,
            },
            packed,
            &[POINTER, SCRATCH, SCRATCH],
        );
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 6,
            ..access(KILLER, 2, place(), 2, SelectedMemoryAccessRole::WritePlace)
        };
    });
    let result = eliminate(&packed, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0].instructions[2].kind,
        SelectedInstructionKind::StorePacked {
            byte_offset: 2,
            width: PackedByteWidth::Six,
        }
    );
    validate_dead_store_elimination(
        &packed,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A one-byte dead store at the tail of the killer's range.
    let tail = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 7,
                byte_size: 1,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0] = SelectedMemoryAccess {
            byte_count: 1,
            ..access(STORE, 1, place(), 7, SelectedMemoryAccessRole::WritePlace)
        };
    });
    eliminate(&tail, &environment).unwrap();
}

/// A write that only partially overlaps the dead range, a packed store too
/// narrow to cover, a row that does not match its instruction's encoded
/// range, a packed store on the wrong constraint, and a dead store of a
/// packed width each leave the dead bytes observable or the pair unproven.
#[test]
fn non_covering_or_mismatched_killers_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A seven-byte packed store leaves the dead store's last byte uncovered.
    let short = mutated(target, |function, environment| {
        let packed = environment
            .constraint(environment.selected_keys().store_packed.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::StorePacked {
                byte_offset: 0,
                width: PackedByteWidth::Seven,
            },
            packed,
            &[POINTER, SCRATCH, SCRATCH],
        );
        function.memory_accesses[1].byte_count = 7;
    });
    assert_eq!(
        eliminate(&short, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A store starting inside the dead range leaves its head bytes live.
    let shifted = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 8,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_offset = 4;
    });
    assert_eq!(
        eliminate(&shifted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A `WritePlace` row that does not match the store's own encoded range
    // cannot cover, whatever the row claims.
    let mismatched = mutated(target, |function, _| {
        function.memory_accesses[1].byte_count = 4;
    });
    assert_eq!(
        eliminate(&mismatched, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A packed store carrying the plain store constraint is not the target's
    // packed-store row.
    let wrong_constraint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::StorePacked {
                byte_offset: 0,
                width: PackedByteWidth::Seven,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_count = 7;
    });
    assert_eq!(
        eliminate(&wrong_constraint, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A `Store` encoding a packed width is not the selected form at all:
    // packed fragment widths select `StorePacked`, so the plain store kind
    // only admits the exact byte sizes.
    let odd_dead = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 3,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0].byte_count = 3;
    });
    assert_eq!(
        eliminate(&odd_dead, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
}

/// The measured work is the block scan, the walked interval, and the roster
/// rows: five block slots, two interval steps through the covering store,
/// and two roster rows measure nine steps for the same-block window; the
/// crossed-edge window scans six slots, measures the crossed tail, the
/// crossed edge, and the covering block — three interval steps — plus the
/// same two rows, eleven steps. Each exact boundary admits the elimination
/// and replays it; one step below rejects the proposal and rejects even the
/// exact correct result under a starved replay budget.
#[test]
fn validation_budget_covers_the_walk() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for (source, exact_steps) in [(fixture(target), 9u64), (chained(target), 11u64)] {
        let exact = OptimizationWorkBudget::new(1, 1, exact_steps, 1, 1).unwrap();
        let result = eliminate_selected_dead_store(&source, 0, STORE, &environment, exact).unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, exact_steps - 1, 1, 1).unwrap();
        assert_eq!(
            eliminate_selected_dead_store(&source, 0, STORE, &environment, starved).unwrap_err(),
            DeadStoreEliminationError::WorkBudgetExceeded
        );
        assert_eq!(
            validate_dead_store_elimination(
                &source,
                0,
                STORE,
                &environment,
                starved,
                result.transformed().clone(),
            )
            .unwrap_err(),
            DeadStoreEliminationError::WorkBudgetExceeded
        );
    }
}

/// A packed `StorePacked` can itself be the dead store: its five bytes at
/// offset 0 sit inside the covering `Store`'s eight, and its early-clobber
/// scratch `Def` occurs nowhere else in the function, so the removal drops
/// the instruction and its write row without orphaning a register. The
/// measured window is unchanged — the same block scan, interval, and roster
/// rows as the plain store — so the same exact budget admits it.
#[test]
fn packed_dead_store_eliminates_under_dead_scratch_custody() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = packed_dead(target);
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // The published plan is a legal second input: the packed store is
        // already gone, and the covering store has no later covering write.
        assert_eq!(
            eliminate_selected_dead_store(&result, 0, STORE, &environment, budget()).unwrap_err(),
            DeadStoreEliminationError::SourceMismatch
        );
        assert_eq!(
            eliminate_selected_dead_store(&result, 0, KILLER, &environment, budget()).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
        let exact = OptimizationWorkBudget::new(1, 1, 9, 1, 1).unwrap();
        eliminate_selected_dead_store(&source, 0, STORE, &environment, exact).unwrap();
        let starved = OptimizationWorkBudget::new(1, 1, 8, 1, 1).unwrap();
        assert_eq!(
            eliminate_selected_dead_store(&source, 0, STORE, &environment, starved).unwrap_err(),
            DeadStoreEliminationError::WorkBudgetExceeded
        );
    }
}

/// The packed removal still carries the whole contract: the constraint must
/// be the target's packed row, the roster row must name the encoded range,
/// and the scratch `Def` must be function-dead — a mention by a later
/// operand, a second definition, or a terminator operand would lose its
/// definition to the removal.
#[test]
fn packed_dead_store_custody_and_surface_reject() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A later use still reads the scratch register.
    let read = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.blocks[0].instructions[2].operands[0].virtual_register = PACKED_SCRATCH;
    });
    assert_eq!(
        eliminate(&read, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A second definition of the scratch register still observes it.
    let redefined = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.blocks[0].instructions[2].operands[1].virtual_register = PACKED_SCRATCH;
    });
    assert_eq!(
        eliminate(&redefined, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A terminator operand naming the scratch register is an occurrence too.
    let terminal = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        let mut mention = function.blocks[0].instructions[0].operands[0];
        mention.virtual_register = PACKED_SCRATCH;
        let selected_instructions::SelectedTerminator::Return { instruction, .. } =
            &mut function.blocks[0].terminator
        else {
            unreachable!()
        };
        instruction.operands.push(mention);
    });
    assert_eq!(
        eliminate(&terminal, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // The packed store on the plain store constraint is not the target's
    // packed-store row.
    let wrong_constraint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::StorePacked {
                byte_offset: 0,
                width: PackedByteWidth::Five,
            },
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0].byte_count = 5;
    });
    assert_eq!(
        eliminate(&wrong_constraint, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // An operand list missing the scratch entry is not the declared surface.
    let dropped_operand = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.blocks[0].instructions[1].operands.remove(2);
    });
    assert_eq!(
        eliminate(&dropped_operand, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A roster row that does not name the packed store's encoded range is
    // not the write's semantic identity.
    let mismatched_row = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.memory_accesses[0].byte_count = 8;
    });
    assert_eq!(
        eliminate(&mismatched_row, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A packed store with no roster row has nothing to prove.
    let rowless = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.memory_accesses.remove(0);
    });
    assert_eq!(
        eliminate(&rowless, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // A packed dead range wider than the covering store leaves bytes
    // observable past the killer's end.
    let uncovered = mutated(target, |function, environment| {
        make_packed_dead(function, environment);
        function.blocks[0].instructions[1].kind = SelectedInstructionKind::StorePacked {
            byte_offset: 4,
            width: PackedByteWidth::Five,
        };
        function.memory_accesses[0].byte_offset = 4;
    });
    assert_eq!(
        eliminate(&uncovered, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// Replay of the packed elimination rejects any drift beyond the exact
/// removal: the dead store retained, a different instruction removed, the
/// dead write row kept, or the covering store's row dropped.
#[test]
fn packed_dead_store_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = packed_dead(target);
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..4 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead packed store must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // A different instruction must not disappear instead.
            1 => {
                function.blocks[0].instructions.remove(1);
            }
            // Kept the dead write row instead of dropping it.
            2 => {
                function.memory_accesses.push(SelectedMemoryAccess {
                    byte_count: 5,
                    ..access(STORE, 1, place(), 0, SelectedMemoryAccessRole::WritePlace)
                });
            }
            // Dropped the covering store's row as well.
            3 => {
                function.memory_accesses.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

/// Replace the dead `Store` with the direct slot store: a `Store64` into
/// `slot` at offset 0, on the target's `store64` row, carrying `WriteLocal`
/// on that slot. The dead range is the row's eight bytes.
fn make_local_dead(
    function: &mut selected_instructions::SelectedFunction,
    slot: LocalStorageSlotId,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) {
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
}

/// A place-storage local write can itself be the dead store: a `Store` or
/// `StorePacked` through the materialized address of the place's own
/// parameter slot, or a `Store64` into that slot directly, each carrying the
/// `WriteLocal` row on it. The covering write is unchanged — the plain
/// `Store` still rewrites the dead range unobserved.
#[test]
fn place_storage_local_dead_writes_eliminate() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The direct `Store64` into the place's parameter home.
        let direct = mutated(target, |function, environment| {
            make_local_dead(
                function,
                LocalStorageSlotId::StructuralParameter { place: place() },
                environment,
            );
        });
        let result = eliminate(&direct, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        // The dropped row is the dead store's `WriteLocal`; the covering
        // `WritePlace` survives.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WritePlace)]
        );
        validate_dead_store_elimination(
            &direct,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A block-parameter slot is the place's storage the same way.
        let block_parameter = mutated(target, |function, environment| {
            make_local_dead(
                function,
                LocalStorageSlotId::StructuralBlockParameter {
                    block: BlockId::new(2).unwrap(),
                    place: place(),
                },
                environment,
            );
        });
        eliminate(&block_parameter, &environment).unwrap();
        // A place `Store` through the parameter slot's materialized address
        // carries the same `WriteLocal` row and is dead the same way.
        let addressed = mutated(target, |function, _| {
            let slot = LocalStorageSlotId::StructuralParameter { place: place() };
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&addressed, &environment).unwrap();
        // The packed dead store may carry the local row too: its scratch
        // custody still has to hold.
        let packed = mutated(target, |function, environment| {
            make_packed_dead(function, environment);
            let slot = LocalStorageSlotId::StructuralParameter { place: place() };
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&packed, &environment).unwrap();
        // Both sides of the pair can take the local route at once: a dead
        // `Store64` covered by another own-slot `Store64`.
        let local_pair = mutated(target, |function, environment| {
            let slot = LocalStorageSlotId::StructuralParameter { place: place() };
            make_local_dead(function, slot, environment);
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result = eliminate(&local_pair, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0]
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                SelectedMemoryAccessRole::WriteLocal {
                    slot: LocalStorageSlotId::StructuralParameter { place: place() },
                }
            )]
        );
        validate_dead_store_elimination(
            &local_pair,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The local-write victim contract stays exact: the row must name the same
/// slot and range the instruction encodes, on the place's own storage. An
/// operation-owned `Structural` slot only stages bytes that name the place,
/// a `WritePlace` row is not the slot store's route, and a second row or a
/// partial cover each still reject.
#[test]
fn local_dead_writes_stay_exact() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `Store64` into an operation-owned `Structural` slot moves staging
    // bytes, not the place's storage: it cannot be the dead store of them.
    let staging = mutated(target, |function, environment| {
        make_local_dead(
            function,
            LocalStorageSlotId::Structural {
                operation: OperationId::new(9).unwrap(),
                place: place(),
            },
            environment,
        );
    });
    assert_eq!(
        eliminate(&staging, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A place `Store` through a staging slot's address is not a write of the
    // place's storage either.
    let staged_store = mutated(target, |function, _| {
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        eliminate(&staged_store, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The direct slot store's row must name the same slot the instruction
    // encodes — here both slots name the dead place, but the row claims the
    // parameter home while the instruction writes the block-parameter slot.
    let wrong_slot = mutated(target, |function, environment| {
        make_local_dead(
            function,
            LocalStorageSlotId::StructuralBlockParameter {
                block: BlockId::new(2).unwrap(),
                place: place(),
            },
            environment,
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal {
            slot: LocalStorageSlotId::StructuralParameter { place: place() },
        };
    });
    assert_eq!(
        eliminate(&wrong_slot, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A `WritePlace` row is not the route a slot store takes.
    let wrong_role = mutated(target, |function, environment| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        make_local_dead(function, slot, environment);
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WritePlace;
    });
    assert_eq!(
        eliminate(&wrong_role, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A row that disagrees with the instruction's encoded range is not the
    // write's semantic identity.
    let shifted_row = mutated(target, |function, environment| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        make_local_dead(function, slot, environment);
        function.memory_accesses[0].byte_offset = 8;
    });
    assert_eq!(
        eliminate(&shifted_row, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The local dead store still needs exactly one row.
    let two_rows = mutated(target, |function, environment| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        make_local_dead(function, slot, environment);
        function.memory_accesses.insert(
            1,
            access(
                STORE,
                9,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ),
        );
    });
    assert_eq!(
        eliminate(&two_rows, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A frame slot outside `Local` storage is not an admitted dead store.
    let incoming = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        function.blocks[0].instructions[1] = instruction(
            STORE,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Incoming {
                    parameter_index: 0,
                    abi_stack_byte_offset: 0,
                },
                byte_offset: 0,
            },
            store64,
            &[VALUE],
        );
    });
    assert_eq!(
        eliminate(&incoming, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
    // The direct slot store on another constraint is not the target's
    // `store64` row.
    let wrong_constraint = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
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
            store,
            &[POINTER, VALUE],
        );
        function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        eliminate(&wrong_constraint, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A covering write that only partially overlaps the local dead range
    // leaves the head bytes observable.
    let uncovered = mutated(target, |function, environment| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        make_local_dead(function, slot, environment);
        function.blocks[0].instructions[3].kind = SelectedInstructionKind::Store {
            byte_offset: 4,
            byte_size: 4,
        };
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_offset: 4,
            byte_count: 4,
            ..access(KILLER, 2, place(), 4, SelectedMemoryAccessRole::WritePlace)
        };
    });
    assert_eq!(
        eliminate(&uncovered, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A boundary settlement inside the local dead store's interval could
    // still observe the dead bytes.
    let settled = mutated(target, |function, environment| {
        let slot = LocalStorageSlotId::StructuralParameter { place: place() };
        make_local_dead(function, slot, environment);
        function.boundary_settlements.push(settlement(3));
    });
    assert_eq!(
        eliminate(&settled, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// Replay of a local-write elimination holds the same exact-removal
/// contract: the dead `Store64` retained, its `WriteLocal` row kept, the
/// covering row dropped, or a surviving instruction altered each drift from
/// the independently derived result.
#[test]
fn local_dead_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let slot = LocalStorageSlotId::StructuralParameter { place: place() };
    let source = mutated(target, |function, environment| {
        make_local_dead(function, slot, environment);
    });
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..4 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead `Store64` must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // Kept the dead `WriteLocal` row instead of dropping it.
            1 => {
                function.memory_accesses.push(access(
                    STORE,
                    1,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WriteLocal { slot },
                ));
            }
            // Dropped the covering store's row as well.
            2 => {
                function.memory_accesses.clear();
            }
            // The surviving `Store` must remain the same place store.
            3 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store {
                    byte_offset: 8,
                    byte_size: 8,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
                .is_err(),
            "mutation {mutation}"
        );
    }
}
