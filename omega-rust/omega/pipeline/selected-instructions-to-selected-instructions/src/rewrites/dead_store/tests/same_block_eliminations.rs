use super::{
    BETWEEN, DEAD_SEQUENCE_INDEX, DEAD_SPAN_COUNT, DEAD_SPAN_CURSOR, KILLER, MATERIALIZE_COUNT,
    MATERIALIZE_INDEX, PACKED_SCRATCH, POINTER, SCRATCH, SEQUENCE_INDEX, SPAN_COUNT, STORE, VALUE,
    access, budget, chained, dead_byte, dead_span_copy, dead_span_length, define_count,
    define_count_as, eliminate, fixture, instruction, make_packed_dead, mutated, packed_dead,
    place, runtime_count, sequence_store, settlement, span_copy, span_length,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::dead_store::{
    DeadStoreEliminationError, eliminate_selected_dead_store, validate_dead_store_elimination,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, PackedByteWidth, SelectedFunction,
    SelectedInstructionId, SelectedInstructionKind, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessRole, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, MachineId, OperationId, PlaceId, ScalarType, ValueId,
};
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

/// An operation-owned `Structural` slot is the place's storage exactly when
/// the place's declaration names that operation as the result's producer —
/// record, case, array, scalar-local, subslice-descriptor, and call-result
/// homes all publish the slot's materialized address as the place's storage
/// pointer, so slot and place share byte coordinates. A write into the
/// producer home interferes and covers like any place-storage write, and a
/// dead store carrying its `WriteLocal` row dies the same way. A slot any
/// other operation writes under the same name only stages bytes that name
/// the place — a call's staged view descriptor — so its writes and
/// materialized address never touch the dead bytes.
#[test]
fn producer_owned_structural_slots_are_place_storage() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let producer = OperationId::new(9).unwrap();
        let slot = LocalStorageSlotId::Structural {
            operation: producer,
            place: place(),
        };
        // The contract declaring `place()` as `producer`'s operation result.
        let declare = |function: &mut SelectedFunction, operation| {
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
        // A covering `Store64` into the producer's home covers the dead
        // range: the slot is the place's storage under the same coordinates.
        let covered = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            declare(function, producer);
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
        let result = eliminate(&covered, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0]
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WriteLocal { slot })]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A dead `Store64` into the producer's home dies the same way: the
        // slot is the place's storage, so its `WriteLocal` row is the dead
        // write the covering place store replaces.
        let dead_local = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            declare(function, producer);
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
        let result = eliminate(&dead_local, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0].blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        validate_dead_store_elimination(
            &dead_local,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // The identical slot under a declaration naming a different producer
        // is a staging slot: the write into it walks past, and with no real
        // cover the dead bytes escape at the boundary.
        let other_producer = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            declare(function, OperationId::new(10).unwrap());
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
        assert_eq!(
            eliminate(&other_producer, &environment).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
        // A partial-range write into the producer's home still leaves the
        // head bytes live.
        let partial = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            declare(function, producer);
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
                byte_count: 4,
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
        // Materializing the producer home's address exposes the place's
        // storage by a route the roster does not bound — it interferes.
        let address = mutated(target, |function, _| {
            declare(function, producer);
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
        // The full staged-descriptor idiom between the dead store and its
        // cover — the staged `WriteLocal` and the materialized staged
        // address — never touches the dead bytes, so both walk past.
        let staged = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            let frame_address = environment
                .constraint(environment.selected_keys().frame_address.unwrap())
                .unwrap();
            function.blocks[0].instructions.insert(
                3,
                instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    },
                    store64,
                    &[VALUE],
                ),
            );
            function.blocks[0].instructions.insert(
                4,
                instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    },
                    frame_address,
                    &[VALUE],
                ),
            );
            function.memory_accesses.push(access(
                SelectedInstructionId(6),
                3,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            ));
            function.memory_accesses.push(SelectedMemoryAccess {
                byte_count: 16,
                ..access(
                    SelectedInstructionId(7),
                    3,
                    place(),
                    0,
                    SelectedMemoryAccessRole::AddressLocal { slot },
                )
            });
        });
        let result = eliminate(&staged, &environment).unwrap();
        validate_dead_store_elimination(
            &staged,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// A staging slot is itself a dead-store subject: a `Store64` into an
/// operation-owned `Structural` slot the place's declaration does not
/// charge to that operation — or a place `Store`/`StorePacked` through the
/// slot's materialized address — writes bytes that only name the place
/// under the slot's own coordinates. The write dies under a later
/// `WriteLocal` covering the same slot's range, and reads or writes of the
/// place's own storage never reach the staged bytes: they walk past, while
/// a same-slot address materialization or a partial cover still rejects.
#[test]
fn staging_slot_dead_writes_die_on_the_slot() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // No structural contract declares the operation the place's
        // producer, so the slot only stages bytes naming it.
        let slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(9).unwrap(),
            place: place(),
        };
        // The direct `Store64` into the staging slot dies under a covering
        // `Store64` into the same slot.
        let direct = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            make_local_dead(function, slot, environment);
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
        // The dropped row is the dead store's `WriteLocal`; the covering
        // `WriteLocal` on the same slot survives.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WriteLocal { slot })]
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
        // A place `Store` through the staging slot's materialized address
        // carries the same `WriteLocal` row and dies the same way — the
        // covering write is a place `Store` addressed the same.
        let addressed = mutated(target, |function, _| {
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
            function.memory_accesses[1].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        let result = eliminate(&addressed, &environment).unwrap();
        assert_eq!(
            result.transformed().functions[0]
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![(KILLER, SelectedMemoryAccessRole::WriteLocal { slot })]
        );
        validate_dead_store_elimination(
            &addressed,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // The packed dead store into the staging slot keeps its scratch
        // custody and dies the same way.
        let packed = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
            make_packed_dead(function, environment);
            function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteLocal { slot };
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
        eliminate(&packed, &environment).unwrap();
        // Reads and writes of the place's own storage between the staging
        // writes never touch the staged bytes: a `ReadPlace` and a
        // `WritePlace` on the named place both walk past.
        let place_rows = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            make_local_dead(function, slot, environment);
            function.memory_accesses.insert(
                1,
                access(BETWEEN, 3, place(), 0, SelectedMemoryAccessRole::ReadPlace),
            );
            function.memory_accesses.push(access(
                BETWEEN,
                3,
                place(),
                8,
                SelectedMemoryAccessRole::WritePlace,
            ));
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&place_rows, &environment).unwrap();
        // A write into a different staging slot of the same place moves
        // other bytes entirely and walks past.
        let other_slot = LocalStorageSlotId::Structural {
            operation: OperationId::new(10).unwrap(),
            place: place(),
        };
        let other_staging = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            make_local_dead(function, slot, environment);
            function.local_storage_slots.push(SelectedLocalStorageSlot {
                id: other_slot,
                byte_size: 16,
                alignment: 8,
            });
            function.blocks[0].instructions[2] = instruction(
                BETWEEN,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(other_slot),
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
                    SelectedMemoryAccessRole::WriteLocal { slot: other_slot },
                ),
            );
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&other_staging, &environment).unwrap();
        // A disjoint same-slot write walks past too: the staging slot's
        // bytes outside the dead range are not the dead bytes.
        let disjoint = mutated(target, |function, environment| {
            let store64 = environment
                .constraint(environment.selected_keys().store64.unwrap())
                .unwrap();
            make_local_dead(function, slot, environment);
            function.blocks[0].instructions[2] = instruction(
                BETWEEN,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 8,
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
                    8,
                    SelectedMemoryAccessRole::WriteLocal { slot },
                ),
            );
            function.blocks[0].instructions[3] = instruction(
                KILLER,
                SelectedInstructionKind::Store64 {
                    slot: FrameStorageSlotId::Local(slot),
                    byte_offset: 0,
                },
                store64,
                &[SCRATCH],
            );
            function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
        });
        eliminate(&disjoint, &environment).unwrap();
    }
}

/// The staging subject's refusal surface: a materialized address of the
/// staging slot exposes its bytes by a route the roster does not bound, a
/// covering write that leaves part of the dead range standing rejects, a
/// write into a different slot never reaches the staged bytes, and a
/// `WritePlace` — a write through the place's referent pointer — cannot
/// cover them either.
#[test]
fn staging_slot_dead_writes_reject_unproven_or_mismatched_covers() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(9).unwrap(),
        place: place(),
    };
    let staging_dead =
        |function: &mut SelectedFunction,
         environment: &register_environment::ValidatedTargetRegisterEnvironment| {
            make_local_dead(function, slot, environment);
        };
    // Materializing the staging slot's address between the dead store and
    // its cover exposes the staged bytes — it interferes without covering.
    let address = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        staging_dead(function, environment);
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
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[2].role = SelectedMemoryAccessRole::WriteLocal { slot };
    });
    assert_eq!(
        eliminate(&address, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A same-slot write that covers only part of the dead range leaves the
    // remaining staged bytes observable.
    let partial = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        staging_dead(function, environment);
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1] = SelectedMemoryAccess {
            byte_count: 4,
            ..access(
                KILLER,
                2,
                place(),
                0,
                SelectedMemoryAccessRole::WriteLocal { slot },
            )
        };
    });
    assert_eq!(
        eliminate(&partial, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The covering write into a different slot — another staging slot of
    // the same place — never touches the dead bytes, so the store's bytes
    // escape at the boundary.
    let other_slot = LocalStorageSlotId::Structural {
        operation: OperationId::new(10).unwrap(),
        place: place(),
    };
    let other = mutated(target, |function, environment| {
        let store64 = environment
            .constraint(environment.selected_keys().store64.unwrap())
            .unwrap();
        staging_dead(function, environment);
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: other_slot,
            byte_size: 16,
            alignment: 8,
        });
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store64 {
                slot: FrameStorageSlotId::Local(other_slot),
                byte_offset: 0,
            },
            store64,
            &[SCRATCH],
        );
        function.memory_accesses[1].role =
            SelectedMemoryAccessRole::WriteLocal { slot: other_slot };
    });
    assert_eq!(
        eliminate(&other, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A `WritePlace` covering row writes the place's storage through its
    // referent pointer — it never reaches the staging slot's bytes.
    let place_cover = mutated(target, |function, environment| {
        staging_dead(function, environment);
    });
    assert_eq!(
        eliminate(&place_cover, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A `WriteLocal` row naming a different place than the slot stages is
    // no coherent staging row.
    let mismatched_place = mutated(target, |function, environment| {
        function.local_storage_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
        make_local_dead(function, slot, environment);
        function.memory_accesses[0].place = PlaceId::new(2).unwrap();
    });
    assert_eq!(
        eliminate(&mismatched_place, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

/// A `WriteLocal` row on the place's own storage decides by range
/// intersection like a `WritePlace` row: a disjoint local write walks past,
/// and an intersecting one still has to cover. A `Structural` operation slot
/// covers only when the place's declaration names that operation as the
/// slot's producer; any other `Structural` slot stages bytes that merely
/// name the place, so its write walks past without covering — and a slot or
/// role that disagrees with the covering instruction rejects.
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
    // An operation-owned `Structural` slot the contract does not charge to
    // the place's producer stages bytes that merely name the place: the
    // covering-range write into it walks past instead of covering, and with
    // no real cover the dead bytes escape at the boundary.
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
        DeadStoreEliminationError::UnsupportedPair
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
    // An operation-owned `Structural` slot write the contract does not charge
    // to the place's producer only stages bytes that name the place — a
    // call's staged view descriptor — so it holds none of the dead bytes and
    // walks past; the covering store still lands.
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
    eliminate(&local, &environment).unwrap();
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

/// A dynamic-extent row's reach is unbounded only upward from its fixed
/// offset — a span covers `length` bytes there and a sequence row touches
/// `offset + index` — so a row on the dead place whose offset begins at or
/// past the dead range's end is provably disjoint and walks past to the
/// covering store, while one starting inside the dead range still
/// interferes however short its recorded reach looks.
#[test]
fn dynamic_extent_rows_past_the_dead_range_walk_past() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let sequence_write = |byte_offset| SelectedMemoryAccess {
        byte_count: 1,
        ..access(
            BETWEEN,
            3,
            place(),
            byte_offset,
            SelectedMemoryAccessRole::WriteByteSequence {
                index: ValueId::new(5).unwrap(),
                value: ValueId::new(6).unwrap(),
                length: ValueId::new(7).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [3; 32],
                ),
            },
        )
    };
    // A byte-sequence write whose payload begins at the dead range's end
    // cannot touch it: the written byte sits at `offset + index`, index >= 0.
    let write_above = mutated(target, |function, _| {
        function.memory_accesses.insert(1, sequence_write(8));
    });
    let result = eliminate(&write_above, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0]
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_offset))
            .collect::<Vec<_>>(),
        vec![(BETWEEN, 8), (KILLER, 0)]
    );
    validate_dead_store_elimination(
        &write_above,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A span write past the dead range walks past the same way.
    let span_write = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 0,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    16,
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
    eliminate(&span_write, &environment).unwrap();
    // A byte-sequence read past the dead range cannot observe it.
    let sequence_read = mutated(target, |function, _| {
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
                byte_count: 1,
                ..access(
                    BETWEEN,
                    3,
                    place(),
                    8,
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
    eliminate(&sequence_read, &environment).unwrap();
    // Starting one byte earlier leaves the dead range's last byte inside
    // the row's upward reach, so the write still interferes.
    let inside = mutated(target, |function, _| {
        function.memory_accesses.insert(1, sequence_write(7));
    });
    assert_eq!(
        eliminate(&inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// The byte-sequence store can itself be the dead store: a `Store { 0, 1 }`
/// through a fully computed view address writes exactly one byte at
/// `offset + index`, and a later byte-sequence store naming the same payload
/// base and the same runtime `index` rewrites that byte unobserved. Any
/// other write shape leaves the dead byte observable — an exact or local
/// range cannot contain a byte whose position is decided at runtime, and a
/// sequence write at another offset or index may land on a different byte
/// entirely.
#[test]
fn byte_sequence_dead_store_dies_under_the_matching_sequence_write() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at `8 + index`; the covering sequence write spells the
        // same byte through the same payload base and index value.
        let covered = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, KILLER]
        );
        // The dead store's `WriteByteSequence` row drops with it; the
        // covering row survives untouched.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                8,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            )]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A different runtime index may land the covering write on a
        // different byte.
        let other_index = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        });
        assert_eq!(
            eliminate(&other_index, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A different payload base places the covering write on another byte
        // the same way.
        let other_offset = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 9, 5, SCRATCH);
        });
        assert_eq!(
            eliminate(&other_offset, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // An exact eight-byte store cannot contain the runtime-placed dead
        // byte however wide its fixed range looks.
        let exact_cover = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 0, 5, VALUE);
        });
        assert_eq!(
            eliminate(&exact_cover, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A byte-sequence read on the dead place can observe the written
        // byte, so it ends the walk before the covering store.
        let observed = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.memory_accesses.insert(
                1,
                SelectedMemoryAccess {
                    byte_count: 1,
                    ..access(
                        BETWEEN,
                        3,
                        place(),
                        16,
                        SelectedMemoryAccessRole::ReadByteSequence {
                            index: ValueId::new(9).unwrap(),
                            length: ValueId::new(7).unwrap(),
                            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                            accepted_fact:
                                optimization_core::AcceptedObligationFactIdentity::from_bytes(
                                    [3; 32],
                                ),
                        },
                    )
                },
            );
        });
        assert_eq!(
            eliminate(&observed, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A covering instruction carrying a second row is not the single
        // sequence write the route admits.
        let two_rows = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.memory_accesses.push(access(
                KILLER,
                9,
                place(),
                0,
                SelectedMemoryAccessRole::ReadPlace,
            ));
        });
        assert_eq!(
            eliminate(&two_rows, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A boundary settlement inside the dead interval could still observe
        // the dead byte before the covering write lands.
        let settled = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.boundary_settlements.push(settlement(3));
        });
        assert_eq!(
            eliminate(&settled, &environment).unwrap_err(),
            DeadStoreEliminationError::InterveningAccess
        );
        // A second roster row on the dead store is not the route's single
        // sequence row.
        let dead_two_rows = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.memory_accesses.push(access(
                STORE,
                9,
                place(),
                0,
                SelectedMemoryAccessRole::ReadPlace,
            ));
        });
        assert_eq!(
            eliminate(&dead_two_rows, &environment).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
        // A sequence row whose recorded count disagrees with the one-byte
        // route is not the byte-sequence store.
        let miscount = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.memory_accesses[0].byte_count = 8;
        });
        assert_eq!(
            eliminate(&miscount, &environment).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
        // A nonzero encoded offset disagrees with the computed-address route
        // the sequence row implies.
        let shifted = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 1,
            };
        });
        assert_eq!(
            eliminate(&shifted, &environment).unwrap_err(),
            DeadStoreEliminationError::UnsupportedPair
        );
    }
}

/// A `CopyBytes` into the dead place is the one covering write whose roster
/// spans several rows: the destination `WriteByteSpan` is a dynamic extent
/// that can still cover an exact dead range when the count register's sole
/// definition is a clean `MaterializeI64` — the span then writes a constant
/// `count` bytes at its fixed `byte_offset`, and containment decides on
/// constants. The copy's source `ReadByteSpan` rides on a disjoint place
/// and stays quiet on the dead bytes.
#[test]
fn byte_span_copy_with_a_constant_count_covers() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead range [0, 8); the copy writes sixteen bytes at offset 0.
        let covered = mutated(target, |function, environment| {
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, MATERIALIZE_COUNT, KILLER]
        );
        assert_eq!(
            function.blocks[0].instructions[3].kind,
            SelectedInstructionKind::CopyBytes
        );
        // The dead write row drops with the store; the covering span and the
        // source read survive untouched.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset, access.role))
                .collect::<Vec<_>>(),
            vec![
                (
                    KILLER,
                    0,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
                (
                    KILLER,
                    0,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
            ]
        );
        validate_dead_store_elimination(
            &covered,
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
        validate_dead_store_elimination(&covered, 0, STORE, &environment, budget(), detached)
            .unwrap();
        // A count equal to the dead range's size covers exactly.
        let exact = mutated(target, |function, environment| {
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 8);
        });
        eliminate(&exact, &environment).unwrap();
        // The span may also begin below the dead range and reach past it.
        let wider = mutated(target, |function, environment| {
            function.memory_accesses[0] = SelectedMemoryAccess {
                byte_offset: 4,
                byte_count: 8,
                ..access(STORE, 1, place(), 4, SelectedMemoryAccessRole::WritePlace)
            };
            function.blocks[0].instructions[1].kind = SelectedInstructionKind::Store {
                byte_offset: 4,
                byte_size: 8,
            };
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 32);
        });
        let result = eliminate(&wider, &environment).unwrap();
        validate_dead_store_elimination(
            &wider,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
    }
}

/// The span covers only while its extent is proven exact: a count that never
/// resolves to a clean `MaterializeI64`, a count short of the dead end, a
/// span beginning inside the dead range, a quiet-violating source read, a
/// second reaching row, or disagreement between the roster's `length` and
/// the count register all leave the dead bytes observable.
#[test]
fn byte_span_covering_requires_the_proven_constant_extent() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A count defined by a copy rather than a `MaterializeI64` resolves no
    // constant — the span's reach stays dynamic and cannot cover.
    let copied = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.virtual_registers.push(VirtualRegister {
            id: SPAN_COUNT,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE_COUNT,
                source_value: span_length(),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.blocks[0].instructions.insert(
            3,
            instruction(
                MATERIALIZE_COUNT,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SPAN_COUNT],
            ),
        );
    });
    assert_eq!(
        eliminate(&copied, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A second definition anywhere in the function breaks the sole clean
    // definition the constant claim rests on.
    let redefined = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            4,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SPAN_COUNT],
            ),
        );
    });
    assert_eq!(
        eliminate(&redefined, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A materialized count short of the dead end leaves the tail bytes live.
    let short = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 4);
    });
    assert_eq!(
        eliminate(&short, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A span beginning inside the dead range can never contain its head.
    let inside = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            4,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
    });
    assert_eq!(
        eliminate(&inside, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A span starting past the dead range is provably disjoint — it walks
    // past, and with no real cover the dead bytes escape at the boundary.
    let disjoint = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            8,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
    });
    assert_eq!(
        eliminate(&disjoint, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The copy's source read on the dead place itself observes the dead
    // bytes before the write rewrites them — a second reaching row the
    // single covering claim cannot describe.
    let observed = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
        function.memory_accesses[2].place = place();
    });
    assert_eq!(
        eliminate(&observed, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A count register naming a different source value is not the span's
    // recorded `length` — the resolved constant would not be the claimed
    // extent.
    let mismatched = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(
            function,
            environment,
            0,
            3,
            SPAN_COUNT,
            ValueId::new(12).unwrap(),
            16,
        );
    });
    assert_eq!(
        eliminate(&mismatched, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A nonzero recorded byte count is not the dynamic span the row claims.
    let miscounted = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
        function.memory_accesses[1].byte_count = 16;
    });
    assert_eq!(
        eliminate(&miscounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The kind without the target's `copy_bytes` row is not the covering
    // route — the count operand is not pinned.
    let foreign = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
        function.blocks[0].instructions[4].constraint = environment.selected_keys().store.unwrap();
    });
    assert_eq!(
        eliminate(&foreign, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A byte-sequence dead store's byte sits at `offset + index` — a fixed
    // span can never contain a runtime-placed position.
    let sequence_dead = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
    });
    assert_eq!(
        eliminate(&sequence_dead, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// Replay accepts only the exact span-covering elimination: the dead store
/// and its write row drop, and the copy's destination span, source read,
/// materialize, and instruction kind all survive untouched.
#[test]
fn byte_span_covering_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 16);
    });
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..6 {
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
            // The covering span's identity must survive intact.
            2 => function.memory_accesses[0].byte_count = 16,
            // Dropped the copy's source read row.
            3 => {
                function.memory_accesses.pop();
            }
            // The surviving killer must remain the `CopyBytes`.
            4 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                };
            }
            // The count's materialize must stay with the copy.
            5 => {
                function.blocks[0].instructions.remove(2);
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

/// A byte-sequence covering store whose `index` resolves to a materialized
/// constant lands on the one fixed byte `byte_offset + index`, so it covers
/// a single-byte dead range exactly when that is the dead byte. The index's
/// constant resolution mirrors the covering span's count: the `index`
/// value's sole `InstructionResult` register must be defined by one clean
/// `MaterializeI64` and never redefined by an edge transport.
#[test]
fn byte_sequence_store_with_a_constant_index_covers() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at offset 8; the covering sequence write's payload base
        // is 4 and its materialized index is 4, landing on byte 8.
        let covered = mutated(target, |function, environment| {
            dead_byte(function, environment, 8);
            sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
            define_count(
                function,
                environment,
                0,
                3,
                SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                4,
            );
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN, MATERIALIZE_COUNT, KILLER]
        );
        // The dead store's `WritePlace` row drops with it; the covering
        // sequence row survives untouched.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                4,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            )]
        );
        validate_dead_store_elimination(
            &covered,
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
        validate_dead_store_elimination(&covered, 0, STORE, &environment, budget(), detached)
            .unwrap();
        // An index of zero places the write exactly at the payload base.
        let at_base = mutated(target, |function, environment| {
            dead_byte(function, environment, 8);
            sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
            define_count(
                function,
                environment,
                0,
                3,
                SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                0,
            );
        });
        eliminate(&at_base, &environment).unwrap();
    }
}

/// The constant-index sequence write covers only while its index is proven
/// constant and lands on the dead byte: an index no instruction result
/// carries, one produced by a copy rather than a `MaterializeI64`, a second
/// definition anywhere in the function, two instruction results claiming
/// the value, a landing byte off the dead byte, a dead range wider than one
/// byte, a recorded count off the one-byte route, a kind or operand surface
/// off the computed-address store, a second roster row on the covering
/// instruction, and a settlement inside the interval each leave the dead
/// byte observable or the pair unproven.
#[test]
fn byte_sequence_covering_requires_the_proven_constant_index() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A runtime index lands the write anywhere at or past the payload base:
    // no instruction result carries the `index` value, so no constant pins
    // the written byte.
    let runtime = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
    });
    assert_eq!(
        eliminate(&runtime, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // An index defined by a copy rather than a `MaterializeI64` resolves no
    // constant — the written byte's position stays runtime-placed.
    let copied = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.virtual_registers.push(VirtualRegister {
            id: SEQUENCE_INDEX,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE_COUNT,
                source_value: ValueId::new(5).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.blocks[0].instructions.insert(
            3,
            instruction(
                MATERIALIZE_COUNT,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SEQUENCE_INDEX],
            ),
        );
    });
    assert_eq!(
        eliminate(&copied, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A second definition anywhere in the function breaks the sole clean
    // definition the constant claim rests on.
    let redefined = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            4,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, SEQUENCE_INDEX],
            ),
        );
    });
    assert_eq!(
        eliminate(&redefined, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // Two instruction results claiming the `index` value leave its producer
    // ambiguous — the resolved constant would not provably be the index.
    let ambiguous = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(15),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: materialize.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(9),
                source_value: ValueId::new(5).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
    });
    assert_eq!(
        eliminate(&ambiguous, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A resolved index landing anywhere but the dead byte leaves it
    // observable: payload base 4 plus index 3 lands on byte 7, not byte 8.
    // The landing is provably off the dead byte, so the row does not even
    // interfere — the walk steps past it and the bytes escape at the
    // return.
    let elsewhere = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
    });
    assert_eq!(
        eliminate(&elsewhere, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // One byte can never cover a wider dead range, even landing on its
    // first byte.
    let wide = mutated(target, |function, environment| {
        sequence_store(function, environment, KILLER, 1, 0, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            0,
        );
    });
    assert_eq!(
        eliminate(&wide, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A recorded count off the one-byte route is not the sequence row the
    // route admits.
    let miscounted = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        function.memory_accesses[1].byte_count = 8;
    });
    assert_eq!(
        eliminate(&miscounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // An encoded offset off `Store { 0, 1 }` is not the computed-address
    // byte-sequence route.
    let shifted = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        function.blocks[0].instructions[4].kind = SelectedInstructionKind::Store {
            byte_offset: 4,
            byte_size: 1,
        };
    });
    assert_eq!(
        eliminate(&shifted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A row off the target's `[use pointer, use value]` store surface is not
    // the byte-sequence route's operand shape.
    let foreign = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        function.blocks[0].instructions[4].constraint = environment.selected_keys().load64.unwrap();
    });
    assert_eq!(
        eliminate(&foreign, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // A covering instruction carrying a second row is not the single
    // sequence write the route admits.
    let two_rows = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        function.memory_accesses.push(access(
            KILLER,
            9,
            place(),
            0,
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        eliminate(&two_rows, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A boundary settlement inside the dead interval could still observe
    // the dead byte before the covering write lands.
    let settled = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
        function.boundary_settlements.push(settlement(4));
    });
    assert_eq!(
        eliminate(&settled, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// Replay accepts only the exact constant-index elimination: the dead store
/// and its write row drop, and the covering sequence row, the index's
/// materialize, and every other instruction survive untouched.
#[test]
fn byte_sequence_covering_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        dead_byte(function, environment, 8);
        sequence_store(function, environment, KILLER, 1, 4, 5, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            4,
        );
    });
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..6 {
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
                function.memory_accesses.push(SelectedMemoryAccess {
                    byte_count: 1,
                    ..access(STORE, 1, place(), 8, SelectedMemoryAccessRole::WritePlace)
                });
            }
            // The covering sequence row's identity must survive intact.
            2 => function.memory_accesses[0].byte_offset = 0,
            // Dropped the covering sequence row.
            3 => {
                function.memory_accesses.pop();
            }
            // The surviving killer must remain the byte-sequence store.
            4 => {
                function.blocks[0].instructions[3].kind = SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                };
            }
            // The index's materialize must stay with the function.
            5 => {
                function.blocks[0].instructions.remove(2);
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

/// A byte-sequence dead store need not share the covering write's `index`
/// value: distinct values still spell the same byte when each resolves to
/// a clean `MaterializeI64` — the same carrier audit the exact-dead-range
/// route runs — because the two `byte_offset + index` sums then name one
/// fixed position apiece. Equal sums cover whatever the payload bases
/// were; unequal sums or an unresolved index stay unproven.
#[test]
fn byte_sequence_dead_store_dies_under_equal_constant_indices() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Dead byte at `8 + 3`; the covering sequence write names a
        // different `index` value whose own materialization is also 3.
        let covered = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            define_count(
                function,
                environment,
                0,
                4,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                3,
            );
        });
        let result = eliminate(&covered, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![
                SelectedInstructionId(1),
                MATERIALIZE_INDEX,
                BETWEEN,
                MATERIALIZE_COUNT,
                KILLER
            ]
        );
        // The dead store's `WriteByteSequence` row drops with it; the
        // covering row survives untouched.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.byte_offset, access.role))
                .collect::<Vec<_>>(),
            vec![(
                KILLER,
                8,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(9).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            )]
        );
        validate_dead_store_elimination(
            &covered,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // The sums decide, not the bases: payload base 9 with index 2
        // lands on the same byte 11 as base 8 with index 3.
        let shifted_base = mutated(target, |function, environment| {
            sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
            sequence_store(function, environment, KILLER, 1, 9, 9, SCRATCH);
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SEQUENCE_INDEX,
                ValueId::new(5).unwrap(),
                3,
            );
            define_count(
                function,
                environment,
                0,
                4,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                2,
            );
        });
        eliminate(&shifted_base, &environment).unwrap();
    }
}

/// Distinct index values cover only while both resolve to constants whose
/// sums land on the dead byte: a covering index landing elsewhere, a dead
/// or covering index no instruction result carries, a carrier defined by
/// a copy rather than a `MaterializeI64`, two carriers claiming one index
/// value, a recorded count off the one-byte route, and a kind off the
/// computed-address store each leave the dead byte observable or the pair
/// unproven.
#[test]
fn byte_sequence_dead_store_rejects_unproven_or_mismatched_constants() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Covering index 4 lands the write on byte 12, not the dead byte 11.
    // The landing is provably off the dead byte, so the row does not even
    // interfere — the walk steps past it and the dead byte escapes at the
    // return.
    let elsewhere = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_count(
            function,
            environment,
            0,
            4,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            4,
        );
    });
    assert_eq!(
        eliminate(&elsewhere, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // The dead index unproduced: no instruction result carries its value,
    // so the dead byte's position stays runtime-placed and the distinct
    // covering index can never be proven to land on it.
    let dead_runtime = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            3,
        );
    });
    assert_eq!(
        eliminate(&dead_runtime, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // The covering index unproduced: no carrier resolves its position.
    let covering_runtime = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
    });
    assert_eq!(
        eliminate(&covering_runtime, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dead-index carrier defined by a copy resolves no constant — the
    // dead byte's position stays runtime-placed.
    let dead_copied = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            3,
        );
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.virtual_registers.push(VirtualRegister {
            id: DEAD_SEQUENCE_INDEX,
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: copy.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: MATERIALIZE_INDEX,
                source_value: ValueId::new(5).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
        function.blocks[0].instructions.insert(
            1,
            instruction(
                MATERIALIZE_INDEX,
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, DEAD_SEQUENCE_INDEX],
            ),
        );
    });
    assert_eq!(
        eliminate(&dead_copied, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // Two instruction results claiming the covering index value leave its
    // producer ambiguous — the resolved constant would not provably be the
    // index.
    let ambiguous = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_count(
            function,
            environment,
            0,
            4,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            3,
        );
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        function.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(15),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
            class: materialize.operands[0].class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(9),
                source_value: ValueId::new(9).unwrap(),
            },
            definition_site: None,
            entry_fixed_view: None,
        });
    });
    assert_eq!(
        eliminate(&ambiguous, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A recorded count off the one-byte route is not the sequence row the
    // cover admits.
    let miscounted = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_count(
            function,
            environment,
            0,
            4,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            3,
        );
        function.memory_accesses[1].byte_count = 8;
    });
    assert_eq!(
        eliminate(&miscounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // An encoded offset off `Store { 0, 1 }` is not the computed-address
    // byte-sequence route.
    let shifted = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 9, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_count(
            function,
            environment,
            0,
            4,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            3,
        );
        function.blocks[0].instructions[5].kind = SelectedInstructionKind::Store {
            byte_offset: 8,
            byte_size: 1,
        };
    });
    assert_eq!(
        eliminate(&shifted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A byte-sequence row whose `index` resolves through the same carrier
/// audit the covering routes run touches exactly one byte —
/// `byte_offset + index` — wherever its payload base sits, so a row
/// landing off the dead extent walks past to the covering write instead
/// of interfering. The dead byte is decided the same way: a dead index
/// resolving to a materialized constant collapses the extent to that one
/// byte, so a resolved row lands off it below or above, and a runtime row
/// whose payload base starts above it cannot reach it. A dead index
/// staying runtime leaves the dead byte anywhere at or past its payload
/// base — only a landing below that base is provably disjoint.
#[test]
fn constant_index_sequence_rows_landing_off_the_dead_extent_walk_past() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Dead byte at `8 + 3` — the dead index materializes, so the extent is
    // byte 11. The intervening sequence write's index materializes to 2
    // against payload base 4, landing on byte 6: it walks past to the
    // covering write landing on byte 11 through the dead index's own
    // value.
    let below = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    let result = eliminate(&below, &environment).unwrap();
    let function = &result.transformed().functions[0];
    assert_eq!(
        function.blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_INDEX,
            MATERIALIZE_COUNT,
            BETWEEN,
            KILLER
        ]
    );
    // The dead store's row drops; the walked-past row and the covering row
    // survive untouched.
    assert_eq!(
        function
            .memory_accesses
            .iter()
            .map(|access| (access.instruction, access.byte_offset, access.role))
            .collect::<Vec<_>>(),
        vec![
            (
                BETWEEN,
                4,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(9).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            ),
            (
                KILLER,
                8,
                SelectedMemoryAccessRole::WriteByteSequence {
                    index: ValueId::new(5).unwrap(),
                    value: ValueId::new(6).unwrap(),
                    length: ValueId::new(7).unwrap(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32]
                    ),
                }
            )
        ]
    );
    validate_dead_store_elimination(
        &below,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A landing above the fixed dead byte is off it the same way: base 4
    // plus index 12 lands on byte 16.
    let above = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            12,
        );
    });
    let result = eliminate(&above, &environment).unwrap();
    validate_dead_store_elimination(
        &above,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A runtime index leaves the intervening row unbounded upward, but a
    // payload base above the fixed dead byte still cannot reach it.
    let runtime_row = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
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
        sequence_store(function, environment, BETWEEN, 1, 16, 9, SCRATCH);
    });
    eliminate(&runtime_row, &environment).unwrap();
    // A dead index staying runtime leaves the dead byte anywhere at or
    // past the payload base — a landing below that base is still provably
    // disjoint.
    let runtime_dead = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    let result = eliminate(&runtime_dead, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), MATERIALIZE_COUNT, BETWEEN, KILLER]
    );
    validate_dead_store_elimination(
        &runtime_dead,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // An exact dead range is met the same way: the intervening sequence
    // write's payload base sits inside it, but its resolved index lands
    // the byte past the range's end.
    let exact_dead = mutated(target, |function, environment| {
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            8,
        );
    });
    let result = eliminate(&exact_dead, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), MATERIALIZE_COUNT, BETWEEN, KILLER]
    );
    validate_dead_store_elimination(
        &exact_dead,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
}

/// The resolved landing is only safe off the dead byte: a read landing on
/// it still observes the stored byte, a write landing on it is the cover
/// itself, a landing at or past an unresolved dead payload base may still
/// meet the runtime-placed byte, and an unresolved index leaves the row's
/// reach unbounded upward into a fixed dead byte.
#[test]
fn constant_index_sequence_rows_still_interfere_on_the_dead_extent() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // Dead byte at `8 + 3` = 11; the intervening sequence read resolves to
    // index 7 against payload base 4 and lands exactly on it.
    let read_on = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
        let load = environment
            .constraint(environment.selected_keys().load8_indexed.unwrap())
            .unwrap();
        function.blocks[0].instructions[4] = instruction(
            BETWEEN,
            SelectedInstructionKind::Load8Indexed,
            load,
            &[POINTER, SEQUENCE_INDEX, SCRATCH],
        );
        function.memory_accesses.insert(
            1,
            SelectedMemoryAccess {
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
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                )
            },
        );
    });
    assert_eq!(
        eliminate(&read_on, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A resolved write landing on the dead byte is the covering write —
    // the walk ends at it and the elimination proceeds with BETWEEN as
    // the cover, the trailing store unscanned.
    let write_on = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            3,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            7,
        );
    });
    let result = eliminate(&write_on, &environment).unwrap();
    assert_eq!(
        result.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![
            SelectedInstructionId(1),
            MATERIALIZE_INDEX,
            MATERIALIZE_COUNT,
            BETWEEN,
            KILLER
        ]
    );
    validate_dead_store_elimination(
        &write_on,
        0,
        STORE,
        &environment,
        budget(),
        result.transformed().clone(),
    )
    .unwrap();
    // A dead index staying runtime leaves the dead byte anywhere at or
    // past the payload base: the intervening write landing on byte 10 may
    // be that byte, so it still interferes.
    let in_reach = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            6,
        );
    });
    assert_eq!(
        eliminate(&in_reach, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // An unresolved index leaves the row's reach unbounded upward: a
    // payload base below the fixed dead byte can still land on it.
    let runtime_index = mutated(target, |function, environment| {
        sequence_store(function, environment, STORE, 0, 8, 5, VALUE);
        sequence_store(function, environment, KILLER, 1, 8, 5, SCRATCH);
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SEQUENCE_INDEX,
            ValueId::new(5).unwrap(),
            3,
        );
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
    });
    assert_eq!(
        eliminate(&runtime_index, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A resolved landing inside an exact dead range is an interfering
    // partial write, not a walk-past.
    let inside_exact = mutated(target, |function, environment| {
        function.memory_accesses.insert(
            1,
            access(BETWEEN, 3, place(), 4, SelectedMemoryAccessRole::WritePlace),
        );
        sequence_store(function, environment, BETWEEN, 1, 4, 9, SCRATCH);
        define_count(
            function,
            environment,
            0,
            2,
            SEQUENCE_INDEX,
            ValueId::new(9).unwrap(),
            2,
        );
    });
    assert_eq!(
        eliminate(&inside_exact, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
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
/// slot and range the instruction encodes. An operation-owned `Structural`
/// slot that only stages bytes naming the place is a staging subject — the
/// dead store of the slot's own bytes — so the place-routed covering store
/// never reaches them and the walk ends at the boundary; a `WritePlace` row
/// is not the slot store's route, and a second row or a partial cover each
/// still reject.
#[test]
fn local_dead_writes_stay_exact() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A `Store64` into an operation-owned `Structural` slot is the dead
    // store of the staging bytes: the `WritePlace` covering store writes
    // the place's storage and never reaches them, so the staged bytes
    // escape at the boundary.
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
    // A place `Store` through a staging slot's address takes the same
    // staging route: the place-routed cover walks past and the bytes
    // escape.
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

/// A `CopyBytes` is itself a dead store: its destination `WriteByteSpan`
/// claims `length` bytes at a fixed `byte_offset`, and once the count
/// register's carrier resolves to a clean `MaterializeI64` the dead extent
/// collapses to that exact range — every covering route then applies
/// unchanged. The copy's whole row set — destination span and source read —
/// drops with the instruction.
#[test]
fn byte_span_dead_store_with_a_constant_count_dies() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // The dead copy writes eight bytes at offset 0; the covering store
        // rewrites [0, 8).
        let source = mutated(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SPAN_COUNT,
                dead_span_length(),
                8,
            );
        });
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), MATERIALIZE_INDEX, BETWEEN, KILLER]
        );
        // The dead copy's destination span and source read both drop; only
        // the covering store's row survives.
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
        // A detached, separately allocated proposal replays by content.
        let mut detached = result.transformed().clone();
        detached.functions = detached.functions.iter().cloned().collect();
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), detached)
            .unwrap();
        // A wider `CopyBytes` whose own count resolves covers the collapsed
        // dead range by containment, exactly like a wider exact store.
        let wider = mutated(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SPAN_COUNT,
                dead_span_length(),
                8,
            );
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                span_length(),
            );
            define_count(function, environment, 0, 4, SPAN_COUNT, span_length(), 32);
        });
        let result = eliminate(&wider, &environment).unwrap();
        let function = &result.transformed().functions[0];
        // The covering copy keeps its destination span and source read.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![
                (
                    KILLER,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
                (
                    KILLER,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
            ]
        );
        validate_dead_store_elimination(
            &wider,
            0,
            STORE,
            &environment,
            budget(),
            result.transformed().clone(),
        )
        .unwrap();
        // A resolved one-byte dead span dies under a byte-sequence write
        // landing on that one byte.
        let sequence = mutated(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            define_count_as(
                function,
                environment,
                0,
                1,
                MATERIALIZE_INDEX,
                DEAD_SPAN_COUNT,
                dead_span_length(),
                1,
            );
            sequence_store(function, environment, KILLER, 1, 0, 9, SCRATCH);
            define_count(
                function,
                environment,
                0,
                4,
                SEQUENCE_INDEX,
                ValueId::new(9).unwrap(),
                0,
            );
        });
        eliminate(&sequence, &environment).unwrap();
    }
}

/// While the dead copy's `length` stays runtime its destination extent is
/// unbounded upward from `byte_offset`: only another `CopyBytes` spelling
/// the same extent — the same `byte_offset` and the same `length` value —
/// provably rewrites every byte it could have written.
#[test]
fn byte_span_dead_store_with_a_runtime_count_dies_under_the_same_extent() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        // Both copies write `dead_span_length()` bytes at offset 0; neither
        // count has a materializing producer, so the extent stays dynamic
        // and identical.
        let source = mutated(target, |function, environment| {
            dead_span_copy(
                function,
                environment,
                0,
                DEAD_SPAN_COUNT,
                dead_span_length(),
            );
            runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
            span_copy(
                function,
                environment,
                KILLER,
                1,
                0,
                SPAN_COUNT,
                dead_span_length(),
            );
            runtime_count(function, environment, SPAN_COUNT, dead_span_length());
        });
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
        // The dead copy's two rows drop; the covering copy's destination
        // span and its source read survive.
        assert_eq!(
            function
                .memory_accesses
                .iter()
                .map(|access| (access.instruction, access.role))
                .collect::<Vec<_>>(),
            vec![
                (
                    KILLER,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
                (
                    KILLER,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    }
                ),
            ]
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
    }
}

/// The `CopyBytes` dead-store route rejects every shape it cannot prove: a
/// resolved count the covering write does not contain, an unresolved extent
/// any bounded or mismatched write cannot bound, a source read reaching the
/// dead destination, a count register that is not the span's `length`, a
/// second write row on the copy, and a scratch register surviving removal
/// would all leave bytes observable or the removal contract unclear.
#[test]
fn byte_span_dead_store_rejects_unproven_or_interfering_shapes() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A resolved dead extent of eight bytes is not contained by the
    // covering store's four.
    let short_cover = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SPAN_COUNT,
            dead_span_length(),
            8,
        );
        function.blocks[0].instructions[4].kind = SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 4,
        };
        function.memory_accesses[1].byte_count = 4;
    });
    assert_eq!(
        eliminate(&short_cover, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // An unresolved dead extent is unbounded upward — no exact write can
    // contain it, however wide.
    let exact_cover = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
    });
    assert_eq!(
        eliminate(&exact_cover, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A covering span claiming a different `length` value never spells the
    // dead extent, even at the same offset.
    let other_length = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        runtime_count(function, environment, SPAN_COUNT, span_length());
    });
    assert_eq!(
        eliminate(&other_length, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A covering span at a shifted offset writes a different extent however
    // the lengths agree.
    let shifted = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            4,
            SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, SPAN_COUNT, dead_span_length());
    });
    assert_eq!(
        eliminate(&shifted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A covering span whose own `length` resolves to a materialized
    // constant still cannot bound an unresolved dead extent — the constant
    // reaches only `byte_offset + count`, and the dead reach extends past
    // it. The covering `length` is a different value from the dead span's,
    // so its resolution leaves the dead extent dynamic.
    let constant_cover = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            span_length(),
        );
        define_count(function, environment, 0, 3, SPAN_COUNT, span_length(), 64);
    });
    assert_eq!(
        eliminate(&constant_cover, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A dead count materialized into a register carrying a different value
    // is not the span's `length` — the resolved constant would not be the
    // written extent.
    let mismatched = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SPAN_COUNT,
            ValueId::new(15).unwrap(),
            8,
        );
    });
    assert_eq!(
        eliminate(&mismatched, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
    // The copy's source read on the dead place itself reaches the dead
    // extent — the removal cannot tell which bytes the copy observed.
    let observed = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        define_count_as(
            function,
            environment,
            0,
            1,
            MATERIALIZE_INDEX,
            DEAD_SPAN_COUNT,
            dead_span_length(),
            8,
        );
        function.memory_accesses[2].place = place();
    });
    assert_eq!(
        eliminate(&observed, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A second write row on the copy is an effect the removal contract does
    // not name.
    let double_write = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, SPAN_COUNT, dead_span_length());
        function.memory_accesses.push(access(
            STORE,
            9,
            PlaceId::new(3).unwrap(),
            0,
            SelectedMemoryAccessRole::WritePlace,
        ));
    });
    assert_eq!(
        eliminate(&double_write, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A scratch register surviving the copy's removal would lose its
    // definition — the custody the packed store's scratch needs.
    let surviving_scratch = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, SPAN_COUNT, dead_span_length());
        let copy = environment
            .constraint(environment.selected_keys().copy_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            2,
            instruction(
                SelectedInstructionId(8),
                SelectedInstructionKind::CopyI64,
                copy,
                &[POINTER, DEAD_SPAN_CURSOR],
            ),
        );
    });
    assert_eq!(
        eliminate(&surviving_scratch, &environment).unwrap_err(),
        DeadStoreEliminationError::ConstraintMismatch
    );
}

/// Replay of a `CopyBytes` dead-store elimination holds the exact-removal
/// contract on the copy's whole row set: the instruction retained, either
/// of its rows kept, the covering row dropped, or a surviving instruction
/// altered each drift from the independently derived result.
#[test]
fn byte_span_dead_store_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        dead_span_copy(
            function,
            environment,
            0,
            DEAD_SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, DEAD_SPAN_COUNT, dead_span_length());
        span_copy(
            function,
            environment,
            KILLER,
            1,
            0,
            SPAN_COUNT,
            dead_span_length(),
        );
        runtime_count(function, environment, SPAN_COUNT, dead_span_length());
    });
    let result = eliminate(&source, &environment).unwrap();
    for mutation in 0..6 {
        let mut proposed = result.transformed().clone();
        let function = &mut proposed.functions[0];
        match mutation {
            // The dead copy must be gone, not retained.
            0 => {
                function.blocks[0].instructions.insert(
                    1,
                    source.transformed().functions[0].blocks[0].instructions[1].clone(),
                );
            }
            // Kept the dead destination span row instead of dropping it.
            1 => {
                function.memory_accesses.push(access(
                    STORE,
                    1,
                    place(),
                    0,
                    SelectedMemoryAccessRole::WriteByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                ));
            }
            // Kept the dead source read row — the copy's second row must
            // drop with the instruction too.
            2 => {
                function.memory_accesses.push(access(
                    STORE,
                    8,
                    PlaceId::new(2).unwrap(),
                    0,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: dead_span_length(),
                        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                        accepted_fact:
                            optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
                    },
                ));
            }
            // Dropped the covering copy's rows as well.
            3 => {
                function.memory_accesses.clear();
            }
            // The surviving copy must remain a `CopyBytes`.
            4 => {
                function.blocks[0].instructions[2].kind = SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                };
            }
            // The covering span's extent identity must survive intact.
            5 => {
                function.memory_accesses[0].role = SelectedMemoryAccessRole::WriteByteSpan {
                    length: span_length(),
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
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
