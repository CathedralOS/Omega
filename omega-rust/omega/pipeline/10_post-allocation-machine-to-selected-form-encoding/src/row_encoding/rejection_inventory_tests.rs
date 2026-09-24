//! Row-level rejection inventory for the common encode/replay route.
//!
//! Every rejection the producer (`encode_row`) and the independent replay
//! (`validation::row::validate`) can raise on one ordinary encoded row is
//! pinned here: header fields, address routing, stored footprint, external
//! operand custody, implicit effects, and declared size. Aggregate
//! counts/identity tamper and the roster-level checks live in
//! `validation/aggregate.rs` and the validated-plan consumers.

use register_model::RegisterUnitId;
use selected_instructions::{MachineSizeKnowledge, SelectedInstructionId};

use super::{SelectedFormEncodingState, encode_row};
use crate::{OptimizedSelectedFormEncodingError, validation};

use super::tests::fixture;

/// The declared implicit effects are part of the machine's contract: an
/// alternative whose recorded effects disagree with what its ISA encoder
/// produced rejects in the producer and in the replay.
#[test]
fn implicit_effect_drift_rejects_in_producer_and_replay() {
    let (physical, selected, mut machine) = fixture();
    machine
        .alternative
        .encoded
        .implicit_unit_defs
        .push(RegisterUnitId(0xdead));
    assert!(matches!(
        encode_row(target::NativeTarget::linux_x64(), &selected, &machine, &physical, None),
        Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(
            instruction
        )) if instruction == SelectedInstructionId(1)
    ));
    let (physical, selected, machine) = fixture();
    let mut mutated = machine.clone();
    mutated
        .alternative
        .encoded
        .implicit_unit_defs
        .push(RegisterUnitId(0xdead));
    let row = encode_row(
        target::NativeTarget::linux_x64(),
        &selected,
        &machine,
        &physical,
        None,
    )
    .unwrap();
    assert!(matches!(
        validation::row::validate(
            target::NativeTarget::linux_x64(),
            &selected,
            &mutated,
            &physical,
            &row
        ),
        Err(OptimizedSelectedFormEncodingError::ImplicitFootprintMismatch(
            instruction
        )) if instruction == SelectedInstructionId(1)
    ));
}

/// External operand indexes name the selected operand roster: renumbering a
/// machine operand so the encoded external list no longer resolves rejects
/// in the producer and in the replay.
#[test]
fn unresolvable_external_operand_rejects_in_producer_and_replay() {
    let (physical, selected, mut machine) = fixture();
    machine.operands[0].operand = 9;
    assert!(matches!(
        encode_row(target::NativeTarget::linux_x64(), &selected, &machine, &physical, None),
        Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(
            instruction
        )) if instruction == SelectedInstructionId(1)
    ));
    let (physical, selected, machine) = fixture();
    let mut mutated = machine.clone();
    mutated.operands[0].operand = 9;
    let row = encode_row(
        target::NativeTarget::linux_x64(),
        &selected,
        &machine,
        &physical,
        None,
    )
    .unwrap();
    assert!(matches!(
        validation::row::validate(
            target::NativeTarget::linux_x64(),
            &selected,
            &mutated,
            &physical,
            &row
        ),
        Err(OptimizedSelectedFormEncodingError::OperandFootprintMismatch(
            instruction
        )) if instruction == SelectedInstructionId(1)
    ));
}

/// The declared size knowledge must bracket the emitted bytes on every leg:
/// an exact declaration off by one and each `EncoderResolved` bound reject
/// in the producer and in the replay, while a bracketing range still admits.
#[test]
fn declared_size_drift_rejects_in_producer_and_replay() {
    let target = target::NativeTarget::linux_x64();
    for knowledge in [
        MachineSizeKnowledge::ExactBytes(9),
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 11,
            maximum_bytes: None,
        },
        MachineSizeKnowledge::EncoderResolved {
            minimum_bytes: 0,
            maximum_bytes: Some(9),
        },
    ] {
        let (physical, selected, mut machine) = fixture();
        machine.alternative.size = knowledge;
        assert!(matches!(
            encode_row(target, &selected, &machine, &physical, None),
            Err(OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
        let (physical, selected, machine) = fixture();
        let mut mutated = machine.clone();
        mutated.alternative.size = knowledge;
        let row = encode_row(target, &selected, &machine, &physical, None).unwrap();
        assert!(matches!(
            validation::row::validate(target, &selected, &mutated, &physical, &row),
            Err(OptimizedSelectedFormEncodingError::SizeDeclarationMismatch(
                instruction
            )) if instruction == SelectedInstructionId(1)
        ));
    }
    let (physical, selected, mut machine) = fixture();
    machine.alternative.size = MachineSizeKnowledge::EncoderResolved {
        minimum_bytes: 1,
        maximum_bytes: Some(15),
    };
    let row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    validation::row::validate(target, &selected, &machine, &physical, &row).unwrap();
}

/// The stored row footprint is a claim, not authority: mutating it after
/// encoding must fail the replay's redecoded-footprint comparison.
#[test]
fn stored_footprint_drift_rejects_in_replay() {
    let (physical, selected, machine) = fixture();
    let target = target::NativeTarget::linux_x64();
    let mut row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    let SelectedFormEncodingState::Encoded { footprint, .. } = &mut row.state else {
        unreachable!()
    };
    footprint.register_reads.push(footprint.register_writes[0]);
    assert!(matches!(
        validation::row::validate(target, &selected, &machine, &physical, &row),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));
}

/// Row header fields bind the row to exactly one instruction, alternative,
/// and disposition: any substitution fails the replay's closed header check.
#[test]
fn row_header_substitutions_reject_in_replay() {
    let (physical, selected, machine) = fixture();
    let target = target::NativeTarget::linux_x64();
    let row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    let mut changed = row.clone();
    changed.instruction = SelectedInstructionId(77);
    assert!(matches!(
        validation::row::validate(target, &selected, &machine, &physical, &changed),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));
    let mut changed = row.clone();
    changed.alternative.variant = 7;
    assert!(matches!(
        validation::row::validate(target, &selected, &machine, &physical, &changed),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));
}

/// A deferred-control kind owns no bytes: substituting an encoded state for
/// its deferred row fails the replay's exact state check.
#[test]
fn encoded_state_on_deferred_control_kind_rejects_in_replay() {
    let (physical, mut selected, machine) = fixture();
    let target = target::NativeTarget::linux_x64();
    let ordinary_row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    selected.kind = selected_instructions::SelectedInstructionKind::Jump;
    let mut row = encode_row(target, &selected, &machine, &physical, None).unwrap();
    assert!(matches!(
        row.state,
        SelectedFormEncodingState::DeferredControl { .. }
    ));
    row.state = ordinary_row.state;
    assert!(matches!(
        validation::row::validate(target, &selected, &machine, &physical, &row),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    ));
}
