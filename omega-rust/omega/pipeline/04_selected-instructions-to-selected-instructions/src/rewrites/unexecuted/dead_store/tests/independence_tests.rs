//! The validator's independence contract: it re-derives the elimination's
//! legality from the source records alone, so a proposal carrying the
//! exact removal a defective producer would publish still fails on the
//! validator's own audit — and a proposal missing the function the
//! contract demands fails the replay comparison. Nothing here consults
//! the producer's `admission` state: the forged proposals are built by
//! hand.

use super::{
    BETWEEN, KILLER, POINTER, SCRATCH, STORE, access, budget, chained, crossed_edge, eliminate,
    fixture, instruction, mutated, mutated_chained, place, settlement,
};
use crate::rewrites::unexecuted::dead_store::{
    DeadStoreEliminationError, ValidatedDeadStoreElimination, validate_dead_store_elimination,
};
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    LocalStorageSlotId, SelectedInstructionKind, SelectedInstructionPlan, SelectedMemoryAccessRole,
    SelectedStructuralBinding, SelectedStructuralTransport,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId};
use target::NativeTarget;

/// The proposal a defective producer would publish for `source`: the named
/// store dropped from its block, its roster rows gone, and the boundary
/// settlements after its position shifted — the mechanical edit with no
/// legality audit behind it, built by hand so the tests below never touch
/// the producer's `admission` constructor or record.
fn forged(source: &ValidatedDeadStoreElimination) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let function = &mut proposed.functions[0];
    let position = function.blocks[0]
        .instructions
        .iter()
        .position(|instruction| instruction.id == STORE)
        .unwrap();
    let block = function.blocks[0].id;
    for settlement in &mut function.boundary_settlements {
        if settlement.block == block && (settlement.instruction_index as usize) > position {
            settlement.instruction_index -= 1;
        }
    }
    function.blocks[0].instructions.remove(position);
    function
        .memory_accesses
        .retain(|access| access.instruction != STORE);
    proposed
}

/// On a legal source the mechanical edit is exactly the proposal the
/// signed result publishes — the same-block fixture and the cross-block
/// chain alike. The forged builder is the contract's edit, so the
/// rejections below come from the validator's own audit rather than a
/// diff quirk, and the signed proposal validates as a second input.
#[test]
fn forged_edit_matches_the_signed_proposal_and_validates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    for source in [fixture(target), chained(target)] {
        let signed = eliminate(&source, &environment).unwrap();
        assert_eq!(&forged(&source), signed.transformed());
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), forged(&source))
            .unwrap();
    }
}

/// A later same-range load still observes the stored bytes: the store is
/// live and its removal is a legality error. A producer that admitted it
/// anyway would publish the mechanical removal while the load's
/// `ReadPlace` row still reaches the vanished bytes — the validator's
/// own walk must refuse with `InterveningAccess`.
#[test]
fn forged_proposal_does_not_launder_an_intervening_read() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let load = environment
            .constraint(environment.selected_keys().load64.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            load,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].role = SelectedMemoryAccessRole::ReadPlace;
    });
    assert_eq!(
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A covering store that writes only half the dead range leaves the tail
/// bytes observable: the proposal's removal still publishes the mechanical
/// edit, and the validator's own walk rejects the partial cover.
#[test]
fn forged_proposal_does_not_launder_a_partial_cover() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        function.blocks[0].instructions[3] = instruction(
            KILLER,
            SelectedInstructionKind::Store {
                byte_offset: 0,
                byte_size: 4,
            },
            store,
            &[POINTER, SCRATCH],
        );
        function.memory_accesses[1].byte_count = 4;
    });
    assert_eq!(
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A call between the dead store and its cover is a barrier the walk
/// cannot cross: the mechanical removal still publishes, and the
/// validator's own audit refuses it.
#[test]
fn forged_proposal_does_not_launder_a_barrier() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
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
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::UnsupportedInstruction
    );
}

/// With no covering write anywhere forward of the dead store the bytes
/// stay observable: the mechanical removal still publishes, and the
/// validator's own walk refuses the uncovered escape.
#[test]
fn forged_proposal_does_not_launder_an_escape() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.blocks[0].instructions.remove(3);
        function.memory_accesses.remove(1);
    });
    assert_eq!(
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::UnsupportedPair
    );
}

/// A boundary settlement positioned inside the dead interval could
/// observe the dead bytes at the boundary: the mechanical removal still
/// publishes, and the validator's own audit refuses it.
#[test]
fn forged_proposal_does_not_launder_a_boundary_settlement() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(3));
    });
    assert_eq!(
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// A structural transport on the crossed edge writes the dead place's
/// storage inside the interval: the mechanical removal still publishes,
/// and the validator's own edge audit refuses it.
#[test]
fn forged_proposal_does_not_launder_a_redefining_edge_transport() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, _| {
        crossed_edge(function)
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
        validate_dead_store_elimination(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source),
        )
        .unwrap_err(),
        DeadStoreEliminationError::InterveningAccess
    );
}

/// On a legal source the contract demands exactly one transformed
/// function: a proposal that keeps the dead store's roster row, drops the
/// covering store too, or leaves a shifted settlement at its source
/// ordinal is not that function — however it was produced.
#[test]
fn validator_rejects_proposals_that_miss_the_demanded_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(4));
    });
    // The store's write row survives the removal.
    let mut kept_row = forged(&source);
    kept_row.functions[0].memory_accesses.push(access(
        STORE,
        1,
        place(),
        0,
        SelectedMemoryAccessRole::WritePlace,
    ));
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), kept_row)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
    // The covering store is removed alongside the dead store.
    let mut dropped_cover = forged(&source);
    dropped_cover.functions[0].blocks[0]
        .instructions
        .retain(|instruction| instruction.id != KILLER);
    dropped_cover.functions[0]
        .memory_accesses
        .retain(|access| access.instruction != KILLER);
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), dropped_cover)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
    // A settlement after the removal still sits at its source ordinal.
    let mut unshifted = forged(&source);
    unshifted.functions[0].boundary_settlements[0].instruction_index = 4;
    assert_eq!(
        validate_dead_store_elimination(&source, 0, STORE, &environment, budget(), unshifted)
            .unwrap_err(),
        DeadStoreEliminationError::ReplayMismatch
    );
}
