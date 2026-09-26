//! Writerless-cycle and disagreeing-leg admission: the walked region is
//! closed under successor edges, so a path that never reaches a covering
//! write is confined forever to blocks that cannot observe the dead bytes.
//! The store dies on those paths too — the bytes are either rewritten
//! before any observer or never read again — while any observation,
//! settlement, transport, or boundary escape inside the region still
//! rejects.

use super::{
    BETWEEN, KILLER, POINTER, SCRATCH, STORE, VALUE, access, budget, eliminate, instruction,
    mutated_chained, place, settlement_at, successor,
};
use crate::rewrites::unexecuted::dead_store::{
    DeadStoreEliminationError, eliminate_selected_dead_store, validate_dead_store_elimination,
};
use optimization_core::OptimizationWorkBudget;
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    LocalStorageSlotId, SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedInstructionId,
    SelectedInstructionKind, SelectedMemoryAccessRole, SelectedStructuralBinding,
    SelectedStructuralTransport, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};
use target::NativeTarget;

/// Replace the chained fixture's successor block body and terminator with a
/// clear self-loop: `block1` holds one pure-register copy and jumps to
/// itself forever. The killer's roster row is dropped with its store.
fn clear_self_loop(
    function: &mut selected_instructions::SelectedFunction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) {
    let jump = environment
        .constraint(environment.selected_keys().jump)
        .unwrap();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    function.blocks[1].instructions = vec![instruction(
        SelectedInstructionId(8),
        SelectedInstructionKind::CopyI64,
        copy,
        &[SCRATCH, SCRATCH],
    )];
    function.blocks[1].terminator = SelectedTerminator::Jump {
        instruction: instruction(
            SelectedInstructionId(9),
            SelectedInstructionKind::Jump,
            jump,
            &[],
        ),
        successor: successor(1),
    };
    function.memory_accesses.remove(1);
}

/// A store whose only path forward spins in a clear self-loop has no
/// covering write anywhere, yet stays dead: the spinning path can never
/// observe the bytes. The removal drops the store and its write row, and
/// replay independently admits the same pair.
#[test]
fn dead_store_dies_into_a_clear_self_loop() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated_chained(target, |function, environment| {
            clear_self_loop(function, environment)
        });
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // The self-loop is retained whole; only the dead store is gone.
        assert_eq!(
            function.blocks[1]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(8)]
        );
        assert!(function.memory_accesses.is_empty());
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

/// Fork legs may disagree about coverage: one leg reaches the covering
/// store while the other spins through a clear two-block cycle that can
/// never observe the bytes. Both paths are dead, so the store is removed.
#[test]
fn dead_store_dies_when_one_leg_covers_and_the_other_cycles_clear() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let environment = baseline_target_register_environment(target).unwrap();
        let source = mutated_chained(target, |function, environment| {
            let branch = environment
                .constraint(environment.selected_keys().conditional_branch)
                .unwrap();
            let jump = environment
                .constraint(environment.selected_keys().jump)
                .unwrap();
            let SelectedTerminator::Jump {
                successor: edge, ..
            } = &function.blocks[0].terminator
            else {
                unreachable!()
            };
            let edge = edge.clone();
            function.blocks[0].terminator = SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(6),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: edge,
                when_zero: successor(2),
            };
            for (id, jump_id, next) in [(2, 8, 3), (3, 9, 2)] {
                function.blocks.push(SelectedBlock {
                    id: SelectedBlockId(id),
                    origin: SelectedBlockOrigin::Source(BlockId::new(u64::from(id) + 1).unwrap()),
                    instructions: Vec::new(),
                    terminator: SelectedTerminator::Jump {
                        instruction: instruction(
                            SelectedInstructionId(jump_id),
                            SelectedInstructionKind::Jump,
                            jump,
                            &[],
                        ),
                        successor: successor(next),
                    },
                });
            }
        });
        let result = eliminate(&source, &environment).unwrap();
        let function = &result.transformed().functions[0];
        assert_eq!(
            function.blocks[0]
                .instructions
                .iter()
                .map(|instruction| instruction.id)
                .collect::<Vec<_>>(),
            vec![SelectedInstructionId(1), BETWEEN]
        );
        // Only the dead store's row is gone; the covering store's stays.
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
    }
}

/// The sharpest disagreement: one clear block whose own legs split — one
/// edge reaches the covering store, the other loops back into itself.
/// Every path either covers or spins unobserving, so the store dies.
#[test]
fn dead_store_dies_when_a_clear_block_splits_between_cover_and_cycle() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, environment| {
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        // Block 0 jumps to a clear block 2 whose legs split: one edge
        // reaches block 1's covering store, the other loops to itself.
        function.blocks[0].terminator = SelectedTerminator::Jump {
            instruction: instruction(
                SelectedInstructionId(6),
                SelectedInstructionKind::Jump,
                jump,
                &[],
            ),
            successor: successor(2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::ConditionalBranch {
                instruction: instruction(
                    SelectedInstructionId(7),
                    SelectedInstructionKind::ConditionalBranchNonZero,
                    branch,
                    &[],
                ),
                when_nonzero: successor(1),
                when_zero: successor(2),
            },
        });
    });
    eliminate(&source, &environment).unwrap();
}

/// Clear-region admission still rejects every observation route the
/// interval could take: a read inside the cycle, a boundary settlement
/// positioned in a spinning block, a structural transport writing the dead
/// place on a crossed edge, an unaccounted store, an escape to the
/// boundary, and a re-entry into the store's own block.
#[test]
fn clear_cycles_still_reject_every_observation_route() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    // A read of the dead place inside the spinning block observes the bytes.
    let reads = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment);
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        ));
        function.memory_accesses.push(access(
            SelectedInstructionId(10),
            4,
            place(),
            0,
            SelectedMemoryAccessRole::ReadPlace,
        ));
    });
    assert_eq!(
        eliminate(&reads, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A boundary settlement inside a never-covering block is an event the
    // boundary could observe the still-current bytes through.
    let settled = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment);
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 0));
    });
    assert_eq!(
        eliminate(&settled, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A structural transport on the edge into the cycle writes the dead
    // place's storage inside the interval.
    let transported = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment);
        let SelectedTerminator::Jump { successor, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        successor
            .structural_bindings
            .push(SelectedStructuralBinding {
                semantic: abstract_operations::AbstractStructuralBinding {
                    parameter: PlaceId::new(2).unwrap(),
                    argument: terminal_psi::StructuralArgument {
                        place: PlaceId::new(2).unwrap(),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    },
                },
                transport: SelectedStructuralTransport::WholeValue {
                    argument: SCRATCH,
                    destination: LocalStorageSlotId::Structural {
                        operation: OperationId::new(9).unwrap(),
                        place: place(),
                    },
                    byte_size: 8,
                    alignment: 8,
                },
            });
    });
    assert_eq!(
        eliminate(&transported, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A store with no roster row inside the cycle is an unaccounted access.
    let unaccounted = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment);
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[1].instructions.push(instruction(
            SelectedInstructionId(10),
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 8,
            },
            store,
            &[POINTER, VALUE],
        ));
    });
    assert_eq!(
        eliminate(&unaccounted, &environment).unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
    // A cycle leg that escapes to a return lets the bytes reach the
    // boundary: the spinning path may be dead, but the escaping one is not.
    let escaping = mutated_chained(target, |function, environment| {
        let jump = environment
            .constraint(environment.selected_keys().jump)
            .unwrap();
        let branch = environment
            .constraint(environment.selected_keys().conditional_branch)
            .unwrap();
        let return_row = environment
            .constraint(environment.selected_keys().return_unit)
            .unwrap();
        clear_self_loop(function, environment);
        // The self-loop becomes a branch: one leg loops, one leg escapes.
        function.blocks[1].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::ConditionalBranchNonZero,
                branch,
                &[],
            ),
            when_nonzero: successor(1),
            when_zero: successor(2),
        };
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(2),
            origin: SelectedBlockOrigin::Source(BlockId::new(3).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Jump {
                instruction: instruction(
                    SelectedInstructionId(10),
                    SelectedInstructionKind::Jump,
                    jump,
                    &[],
                ),
                successor: successor(3),
            },
        });
        function.blocks.push(SelectedBlock {
            id: SelectedBlockId(3),
            origin: SelectedBlockOrigin::Source(BlockId::new(4).unwrap()),
            instructions: Vec::new(),
            terminator: SelectedTerminator::Return {
                instruction: instruction(
                    SelectedInstructionId(11),
                    SelectedInstructionKind::ReturnUnit,
                    return_row,
                    &[],
                ),
                psi_return_edge: EdgeId::new(3).unwrap(),
            },
        });
    });
    assert_eq!(
        eliminate(&escaping, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
    // A cycle edge back into the store's own block re-executes the removed
    // store from its top — never admissible.
    let reentering = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment);
        let SelectedTerminator::Jump {
            successor: edge, ..
        } = &mut function.blocks[1].terminator
        else {
            unreachable!()
        };
        *edge = successor(0);
    });
    assert_eq!(
        eliminate(&reentering, &environment).unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

/// Replay independently re-derives the closed-region admission: a proposal
/// that drifts from the source-minus-store shape — a surviving write row,
/// a mutated cycle body, or a missing settlement shift — mismatches.
#[test]
fn clear_cycle_replay_rejects_mutated_proposals() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment)
    });
    let result = eliminate(&source, &environment).unwrap();
    // A proposal that keeps the dead write's roster row mismatches.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].memory_accesses.push(access(
        STORE,
        1,
        place(),
        0,
        SelectedMemoryAccessRole::WritePlace,
    ));
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
    // A proposal that also removed the cycle's body instruction mismatches.
    let mut proposed = result.transformed().clone();
    proposed.functions[0].blocks[1].instructions.clear();
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), proposed)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
}

/// The closed-region admission is deterministic and terminal: two runs over
/// the identical source produce the identical validated result, and the
/// published plan is a legal fixed point — the removed store is gone and
/// nothing else in the function can die.
#[test]
fn clear_cycle_elimination_is_deterministic_and_terminal() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment)
    });
    let first = eliminate(&source, &environment).unwrap();
    let second = eliminate(&source, &environment).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        eliminate_selected_dead_store(&first, 0, STORE, &environment, budget()).unwrap_err(),
        DeadStoreEliminationError::SourceMismatch
    );
}

/// The closed-region walk charges the same measured window: total block
/// instructions, the scanned interval, the crossed edges, and the roster.
/// The self-loop fixture measures eleven validation steps exactly.
#[test]
fn clear_cycle_admission_charges_the_measured_window() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, environment| {
        clear_self_loop(function, environment)
    });
    let exact = OptimizationWorkBudget::new(1, 1, 11, 1, 1).unwrap();
    eliminate_selected_dead_store(&source, 0, STORE, &environment, exact).unwrap();
    let starved = OptimizationWorkBudget::new(1, 1, 10, 1, 1).unwrap();
    assert_eq!(
        eliminate_selected_dead_store(&source, 0, STORE, &environment, starved).unwrap_err(),
        DeadStoreEliminationError::WorkBudgetExceeded
    );
}
