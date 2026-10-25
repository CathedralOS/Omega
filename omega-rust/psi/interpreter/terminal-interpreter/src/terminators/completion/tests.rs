//! Ownership fault controls; source-produced embedding/corpus tests establish
//! language behavior. The minimal owner fixture bypasses artifact admission.
use super::*;
use crate::byte_sequences::{
    ByteSequenceView, binding::ByteSequenceBinding, storage::ByteSequenceStorage,
};
use crate::calls::frames::tests::empty_program;
use crate::execution::ResultClaimOrigin;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationMeasurementIdentity,
    TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::{ClaimId, StructuralTypeId};
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralResultClaimBinding,
};

#[test]
fn refused_return_descriptor_retains_detached_backing() {
    check_refused_return(false);
    check_refused_return(true);
}

fn check_refused_return(retain_header: bool) {
    let measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    let sponsor = BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 2, 100, 100, 100).unwrap(),
        measurement,
    )
    .unwrap();
    let account = sponsor.begin_invocation(measurement, 100).unwrap();
    let mut execution = TerminalExecution::start_with_program(
        empty_program(),
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        None,
        Some(account.clone()),
    )
    .unwrap();
    let claim = ClaimId::new(1).unwrap();
    let claims = vec![StructuralResultClaimBinding {
        claim,
        path: Vec::new(),
    }];
    let structural_type = StructuralTypeId::new(1).unwrap();
    let result = StructuralOperationResult {
        place: PlaceId::new(1).unwrap(),
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: claims.clone(),
    };
    execution
        .suspend_with(
            SuspendedCallResult::Structural {
                result: result.clone(),
                claim_origin: ResultClaimOrigin::CalleeReturn,
                expected_reference_backings: Vec::new(),
                profile_completion: None,
            },
            Vec::new(),
        )
        .unwrap();
    // Hold the existing header reservation outside the frame to deterministically
    // deny even the first row. This injects the same budget pressure as another
    // sponsored activation taking the freed cell, without a timing-based race.
    let retained_header = if retain_header {
        execution
            .call_stack
            .last_mut()
            .unwrap()
            ._frame_storage
            .take()
    } else {
        None
    };
    let mut outgoing = CompletedReturn::plain(TerminalExecutionResult::Structural(
        TerminalStructuralResult {
            value: TerminalStructuralValue::minted(42, structural_type, Vec::new()),
            claims: vec![claim],
        },
    ));
    outgoing.destination = Some(result.clone());
    outgoing.returned_claims = claims.clone();
    outgoing.byte_sequence = Some(ByteSequenceBinding::Immutable(ByteSequenceView::new(
        ByteSequenceStorage::copy(b"retained", Some(&account)).unwrap(),
    )));
    // Restoring the caller releases one header. Its structural row consumes
    // that cell, then the descriptor insertion must refuse without losing bytes.
    let error = execution
        .finish_return(outgoing)
        .err()
        .expect("descriptor refusal");
    assert!(
        format!("{error:?}").contains("AllowanceExhausted(LiveStorage)"),
        "{error:?}"
    );
    assert_eq!(
        sponsor.live_payload_bytes(),
        8,
        "detached return stays with failed activation"
    );
    assert_eq!(sponsor.live_storage_cells(), 2);
    assert_eq!(
        execution.structural_values.len(),
        usize::from(!retain_header)
    );
    assert!(execution.byte_sequence_values.is_empty());
    let failed = execution
        .failed_return
        .as_ref()
        .expect("failed return owner");
    let retained = &failed._completed;
    assert_eq!(retained.returned_claims, claims);
    assert_eq!(retained.destination.as_ref(), Some(&result));
    let TerminalExecutionResult::Structural(value) = &retained.value else {
        panic!("returned carrier lost");
    };
    assert_eq!(value.claims, [claim]);
    assert_eq!(value.value.opaque_identity, 42);
    assert!(
        matches!(failed._expected.as_ref(), Some(SuspendedCallResult::Structural { result: retained, .. }) if retained == &result)
    );
    assert!(failed._loan_completions.is_empty());
    let Some(ByteSequenceBinding::Immutable(bytes)) = &retained.byte_sequence else {
        panic!("returned bytes lost");
    };
    assert_eq!(bytes.bytes(), b"retained");
    drop(execution);
    assert_eq!(sponsor.live_payload_bytes(), 0);
    assert_eq!(sponsor.live_storage_cells(), u64::from(retain_header));
    drop(retained_header);
    assert_eq!(sponsor.live_storage_cells(), 0);
}
