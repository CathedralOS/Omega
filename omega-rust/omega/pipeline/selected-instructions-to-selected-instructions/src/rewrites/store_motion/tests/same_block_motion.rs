use super::{
    BETWEEN, KILLER, POINTER, SCRATCH, STORE, VALUE, access, budget, chained, fixture, instruction,
    landed_ids, mutated, place, settlement, sink,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::store_motion::{
    StoreMutationMotionError, sink_selected_store_mutation, validate_store_mutation_motion,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    LocalStorageSlotId, SelectedInstructionId, SelectedInstructionKind, SelectedMemoryAccess,
    SelectedMemoryAccessRole,
};
use semantic_vocabulary::{MachineId, OperationId, ScalarType, ValueId};
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
    // A dynamic-extent write to the same place cannot be proven disjoint, so
    // it bounds the motion just as an exact row does.
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
    // A place-backed local slot write targets the moved place's storage.
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
        sink(&local, &environment).unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
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
