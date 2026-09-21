//! The validator's independence contract: it re-derives the forwarding's
//! legality from the source records alone, so a proposal carrying the exact
//! edit a defective producer would publish still fails on the validator's
//! own audit — and a proposal missing the function the contract demands
//! fails the replay comparison. Nothing here consults the producer's
//! `admission` state: the forged proposals are built by hand.
use super::{
    LOAD, OUTPUT, POINTER, SCRATCH, VALUE, budget, crossed_edge, fixture, forward, instruction,
    mutated, mutated_chained,
};
use crate::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, validate_stored_load_forwarding,
};
use register_environment::{
    ValidatedTargetRegisterEnvironment, baseline_target_register_environment,
};
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan, SelectedValueBinding,
    SelectedValueTransport,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId};
use target::NativeTarget;

/// The proposal a defective producer would publish for `source`: the named
/// load replaced by the contract's `CopyI64` of the stored register —
/// identity, operand classes, and the read's provenance carried — and its
/// read row dropped from the roster. This is the mechanical edit with no
/// legality audit behind it, built by hand so the tests below never touch
/// the producer's `admission` constructor or record.
fn forged(
    source: &ValidatedStoredLoadForwarding,
    environment: &ValidatedTargetRegisterEnvironment,
) -> SelectedInstructionPlan {
    let mut proposed = source.transformed().clone();
    let function = &mut proposed.functions[0];
    let (block, position) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, candidate)| {
            candidate
                .instructions
                .iter()
                .position(|instruction| instruction.id == LOAD)
                .map(|position| (block, position))
        })
        .unwrap();
    let copy = environment
        .constraint(environment.selected_keys().copy_i64)
        .unwrap();
    let mut replacement = instruction(
        LOAD,
        SelectedInstructionKind::CopyI64,
        copy,
        &[VALUE, OUTPUT],
    );
    replacement.provenance = function.blocks[block].instructions[position]
        .provenance
        .clone();
    function.blocks[block].instructions[position] = replacement;
    let row = function
        .memory_accesses
        .iter()
        .position(|access| access.instruction == LOAD)
        .unwrap();
    function.memory_accesses.remove(row);
    proposed
}

/// The hand-forged edit is byte-for-byte the plan the producer signs for a
/// legal pair, and the validator replays it standalone: both sides derive
/// the same demanded function from the source alone.
#[test]
fn forged_edit_matches_the_signed_proposal_and_validates() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    let signed = forward(&source, &environment).unwrap();
    assert_eq!(&forged(&source, &environment), signed.transformed());
    let replayed = validate_stored_load_forwarding(
        &source,
        0,
        LOAD,
        &environment,
        budget(),
        forged(&source, &environment),
    )
    .unwrap();
    assert_eq!(replayed.transformed(), signed.transformed());
}

/// An unaccounted referent store between the covering store and the load
/// makes the pair illegal — the producer's own walk refuses it — yet a
/// defective producer that skipped the roster accounting would publish the
/// very proposal `forged` builds. The validator's own walk must refuse the
/// source with the legality error, not merely diff the proposal.
#[test]
fn forged_proposal_does_not_launder_an_unaccounted_write() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let store = environment
            .constraint(environment.selected_keys().store.unwrap())
            .unwrap();
        // The extra store carries no roster row: an unaccounted write.
        function.blocks[0].instructions.insert(
            2,
            instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::Store {
                    byte_offset: 0,
                    byte_size: 8,
                },
                store,
                &[POINTER, SCRATCH],
            ),
        );
    });
    assert_eq!(
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            forged(&source, &environment),
        )
        .unwrap_err(),
        StoredLoadForwardingError::AliasingWrite
    );
}

/// A materialized constant redefining the carried register between the
/// store and the load passes the roster accounting — it writes no storage —
/// but kills the forwarding the moment the span audit runs. The defective
/// edit is identical either way; only the validator's own register
/// preservation audit sees the value die.
#[test]
fn forged_proposal_does_not_launder_a_redefined_carried_register() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = mutated(target, |function, environment| {
        let materialize = environment
            .constraint(environment.selected_keys().materialize_i64)
            .unwrap();
        function.blocks[0].instructions.insert(
            2,
            instruction(
                SelectedInstructionId(9),
                SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(7),
                },
                materialize,
                &[VALUE],
            ),
        );
    });
    assert_eq!(
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            forged(&source, &environment),
        )
        .unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
}

/// An edge transport redefining the carried register is crossed-edge
/// legality the proposal cannot show: the chained fixture's forged edit
/// replaces the load in its own block exactly as the contract demands, and
/// the validator's own edge audit still refuses it.
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
                parameter: VALUE,
            },
        });
    });
    assert_eq!(
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            forged(&source, &environment),
        )
        .unwrap_err(),
        StoredLoadForwardingError::UnsupportedUse
    );
}

/// A legal source whose proposal misses the demanded function — the load
/// kept, the wrong register carried, the read row retained, or provenance
/// the read never wore — is a replay mismatch, not a legality error: the
/// source is clean, so only the proposal comparison can refuse it.
#[test]
fn validator_rejects_proposals_that_miss_the_demanded_function() {
    let target = NativeTarget::linux_x64();
    let environment = baseline_target_register_environment(target).unwrap();
    let source = fixture(target);
    // The unchanged source keeps the load and its read row.
    assert_eq!(
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            source.transformed().clone(),
        )
        .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
    // A copy of the wrong register is not the demanded replacement.
    let mut wrong_register = forged(&source, &environment);
    wrong_register.functions[0].blocks[0].instructions[3].operands[0].virtual_register = SCRATCH;
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), wrong_register,)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
    // The right replacement with the read row retained misses the roster.
    let mut kept_row = forged(&source, &environment);
    let read_row = source.transformed().functions[0].memory_accesses[1].clone();
    kept_row.functions[0].memory_accesses.push(read_row);
    assert_eq!(
        validate_stored_load_forwarding(&source, 0, LOAD, &environment, budget(), kept_row,)
            .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
    // The contract's copy carries the read's provenance; a replacement
    // without it is not the instruction the validator reconstructs.
    let mut wrong_provenance = forged(&source, &environment);
    wrong_provenance.functions[0].blocks[0].instructions[3].provenance = Default::default();
    assert_eq!(
        validate_stored_load_forwarding(
            &source,
            0,
            LOAD,
            &environment,
            budget(),
            wrong_provenance,
        )
        .unwrap_err(),
        StoredLoadForwardingError::ReplayMismatch
    );
}
