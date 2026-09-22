//! The validator's independence contract: it re-derives the motion's
//! legality from the source records alone, so a proposal carrying the
//! exact move a defective producer would publish still fails on the
//! validator's own audit — and a proposal missing the function the
//! contract demands fails the replay comparison. Nothing here consults
//! the producer's `admission` state: the forged proposals are built by
//! hand.
use super::{
    BETWEEN, POINTER, SCRATCH, STORE, VALUE, budget, chained, crossed_edge, fixture, instruction,
    mutated, mutated_chained, settlement, settlement_at, sink,
};
use crate::rewrites::unexecuted::{
    StoreMutationMotionError, ValidatedStoreMutationMotion, validate_store_mutation_motion,
};
use register_environment::baseline_target_register_environment;
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedValueBinding, SelectedValueTransport,
};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
use target::NativeTarget;

/// The proposal a defective producer would publish for `source`: the
/// named store moved from its position in `source_block` to
/// `insert_index` in `target_block`, the roster and settlements carried
/// unchanged. This is the mechanical edit with no legality audit behind
/// it, built by hand so the tests below never touch the producer's
/// `admission` constructor or record.
fn forged(
    source: &ValidatedStoreMutationMotion,
    source_block: usize,
    target_block: usize,
    insert_index: usize,
) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let function = &mut proposed.functions[0];
    let position = function.blocks[source_block]
        .instructions
        .iter()
        .position(|instruction| instruction.id == STORE)
        .unwrap();
    let moved = function.blocks[source_block].instructions.remove(position);
    function.blocks[target_block]
        .instructions
        .insert(insert_index, moved);
    proposed
}

/// The hand-forged edits are byte-for-byte the plans the producer signs
/// for legal sources — the store landed immediately before the covering
/// store in the same block, or across the sole edge at the successor's
/// head — and the validator replays them standalone: both sides derive
/// the same demanded function from the source alone.
#[test]
fn forged_edit_matches_the_signed_proposal_and_validates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let signed = sink(&source, &environment).unwrap();
    assert_eq!(&forged(&source, 0, 0, 2), signed.transformed());
    let replayed = validate_store_mutation_motion(
        &source,
        0,
        STORE,
        &environment,
        budget(),
        forged(&source, 0, 0, 2),
    )
    .unwrap();
    assert_eq!(replayed.transformed(), signed.transformed());
    let chained_source = chained(target);
    let signed = sink(&chained_source, &environment).unwrap();
    assert_eq!(&forged(&chained_source, 0, 1, 0), signed.transformed());
    let replayed = validate_store_mutation_motion(
        &chained_source,
        0,
        STORE,
        &environment,
        budget(),
        forged(&chained_source, 0, 1, 0),
    )
    .unwrap();
    assert_eq!(replayed.transformed(), signed.transformed());
}

/// An unaccounted referent store immediately after the moved store makes
/// the motion illegal — the only provable landing is the store's own
/// index, which admits nothing — yet a defective producer that skipped
/// the roster accounting would publish the very proposal `forged`
/// builds. The validator's own walk must refuse the source with the
/// legality error, not merely diff the proposal.
#[test]
fn forged_proposal_does_not_launder_an_unaccounted_write() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        // The next instruction carries no roster row: an unaccounted
        // write the slide cannot cross.
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
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 2),
        )
        .unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// A call immediately after the moved store observes through routes the
/// roster cannot see, so the store can never cross it — and the same
/// defective slide that ignores the barrier publishes the forged edit.
/// The validator's own barrier audit must refuse.
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
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 2),
        )
        .unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// A register redefinition immediately after the store writes no storage
/// — it passes the roster accounting — but it kills the motion the
/// moment the coupling audit runs: the moved store reads `VALUE`, so the
/// copy's `Def` must stay ordered after it. The defective edit is
/// identical either way; only the validator's own coupling audit sees
/// the hazard.
#[test]
fn forged_proposal_does_not_launder_a_redefined_carried_register() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
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
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 2),
        )
        .unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// A boundary settlement at the store's next position would observe the
/// place before the moved write, so nothing later is reachable — the
/// store cannot move at all. A producer that forgot the settlement check
/// still emits the forged slide; the validator's own settlement audit
/// refuses it.
#[test]
fn forged_proposal_does_not_launder_a_boundary_settlement() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, _| {
        function.boundary_settlements.push(settlement(2));
    });
    assert_eq!(
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 2),
        )
        .unwrap_err(),
        StoreMutationMotionError::UnsupportedPair
    );
}

/// An edge transport redefining a carried register is crossed-edge
/// legality the proposal cannot show: the chained source admits only the
/// block-end landing — the edge audit stops the crossing before the
/// successor — so a defective producer that skipped the transports would
/// publish the forged crossing into block 1. The validator's own edge
/// audit lands the store at the crossed block's end and the forged
/// proposal mismatches the demanded function.
#[test]
fn forged_proposal_does_not_launder_a_redefining_edge_transport() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated_chained(target, |function, _| {
        crossed_edge(function).bindings.push(SelectedValueBinding {
            semantic: abstract_operations::ValueBinding {
                parameter: ValueId::new(5).unwrap(),
                argument: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            transport: SelectedValueTransport::Registers {
                argument: SCRATCH,
                parameter: POINTER,
            },
        });
    });
    // The demanded edit lands the store at block 0's end: replaying it
    // standalone proves the source itself is legal.
    let replayed = validate_store_mutation_motion(
        &source,
        0,
        STORE,
        &environment,
        budget(),
        forged(&source, 0, 0, 2),
    )
    .unwrap();
    assert_eq!(
        replayed.transformed().functions[0].blocks[0]
            .instructions
            .iter()
            .map(|instruction| instruction.id)
            .collect::<Vec<_>>(),
        vec![SelectedInstructionId(1), BETWEEN, STORE]
    );
    // The forged crossing — the proposal a defective producer would
    // publish — is not the function the contract demands.
    assert_eq!(
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 1, 0),
        )
        .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
}

/// A legal source whose proposal misses the demanded function — the
/// store unmoved or landed off the proven position, the roster edited,
/// or the settlements remapped wrong — is a replay mismatch, not a
/// legality error: the source is clean, so only the proposal comparison
/// can refuse it.
#[test]
fn validator_rejects_proposals_that_miss_the_demanded_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The unchanged source keeps the store in place; the contract
    // demands it before the covering store.
    assert_eq!(
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
    // Landing after the covering store instead of before it is not the
    // proven position.
    assert_eq!(
        validate_store_mutation_motion(
            &source,
            0,
            STORE,
            &environment,
            budget(),
            forged(&source, 0, 0, 3),
        )
        .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
    // The roster names instructions by identity and is retained
    // unchanged; a dropped row is not the demanded function.
    let mut dropped_row = forged(&source, 0, 0, 2);
    dropped_row.functions[0].memory_accesses.pop();
    assert_eq!(
        validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), dropped_row,)
            .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
    // A phantom settlement cannot appear in the proposal.
    let mut phantom = forged(&source, 0, 0, 2);
    phantom.functions[0]
        .boundary_settlements
        .push(settlement(0));
    assert_eq!(
        validate_store_mutation_motion(&source, 0, STORE, &environment, budget(), phantom,)
            .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
    // Across the edge the demanded settlement remap shifts the landing
    // block's ordinals one later; a proposal that moves the store but
    // keeps the settlement's position mismatches.
    let head = mutated_chained(target, |function, _| {
        function
            .boundary_settlements
            .push(settlement_at(SelectedBlockId(1), 0));
    });
    assert_eq!(
        validate_store_mutation_motion(
            &head,
            0,
            STORE,
            &environment,
            budget(),
            forged(&head, 0, 1, 0),
        )
        .unwrap_err(),
        StoreMutationMotionError::ReplayMismatch
    );
    let mut shifted = forged(&head, 0, 1, 0);
    shifted.functions[0].boundary_settlements[0].instruction_index = 1;
    validate_store_mutation_motion(&head, 0, STORE, &environment, budget(), shifted).unwrap();
}
