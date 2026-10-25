use super::*;
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, EvaluationAccountingError,
    EvaluationMeasurementIdentity, EvaluationResource, TerminalStorageScheduleIdentity,
};

fn sponsor(cells: u64) -> BuildEvaluationSponsor {
    BuildEvaluationSponsor::new_with_measurement(
        BuildEvaluationSponsorLimits::new(100, 8, 2, 2, cells, 100, 100, 100).unwrap(),
        EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule: terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        },
    )
    .unwrap()
}

#[test]
fn payload_is_funded_before_reading_elements_and_refusal_preserves_inspection() {
    let sponsor = sponsor(2);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut value = TerminalScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [TerminalScalarValue::Boolean(false); 2].map(Ok),
        Some(&account),
    )
    .unwrap();
    let inspected = value.clone();
    assert_eq!(sponsor.live_storage_cells(), 2);
    assert!(matches!(
        value.elements_mut(),
        Err(TerminalInterpretError::Storage(
            EvaluationAccountingError::AllowanceExhausted(EvaluationResource::LiveStorage)
        ))
    ));
    assert_eq!(value.elements(), inspected.elements());
    assert_eq!(sponsor.live_storage_cells(), 2);
    assert!(value.snapshot().is_err());
    let consumed = std::cell::Cell::new(false);
    assert!(
        ScalarArrayStorage::collect(
            1,
            std::iter::once_with(|| {
                consumed.set(true);
                Ok(TerminalScalarValue::Boolean(true))
            }),
            Some(&account)
        )
        .is_err()
    );
    assert!(
        !consumed.get(),
        "refusal must precede payload evaluation/allocation"
    );
    drop(inspected);
    value.elements_mut().unwrap()[0] = TerminalScalarValue::Boolean(true);
    assert_eq!(sponsor.live_storage_cells(), 2, "unique writes do not copy");
    drop(value);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn snapshots_and_detached_writes_own_their_overlap_until_last_observation() {
    let sponsor = sponsor(4);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let mut value = TerminalScalarArrayValue::collect(
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        2,
        [TerminalScalarValue::Boolean(false); 2].map(Ok),
        Some(&account),
    )
    .unwrap();
    value.storage_identity = Some(7);
    let copied = value.snapshot().unwrap();
    assert_eq!(copied.storage_identity, None);
    assert_eq!(copied.elements(), value.elements());
    assert_eq!(sponsor.live_storage_cells(), 4);
    drop(copied);
    let inspected = value.clone();
    value.elements_mut().unwrap()[0] = TerminalScalarValue::Boolean(true);
    assert_eq!(value.storage_identity, Some(7));
    assert_eq!(
        inspected.elements(),
        [TerminalScalarValue::Boolean(false); 2]
    );
    assert_eq!(sponsor.live_storage_cells(), 4);
    assert_eq!(sponsor.peak_live_cells(), 4);
    drop(value);
    assert_eq!(sponsor.live_storage_cells(), 2);
    drop(inspected);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn empty_and_incomplete_payloads_do_not_leak_reservations() {
    let sponsor = sponsor(2);
    let account = sponsor
        .begin_invocation(sponsor.measurement(), 100)
        .unwrap();
    let empty = ScalarArrayStorage::collect(0, [], Some(&account)).unwrap();
    assert!(empty.elements().is_empty());
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert!(ScalarArrayStorage::collect(2, [], Some(&account)).is_err());
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert!(
        ScalarArrayStorage::collect(0, [Ok(TerminalScalarValue::Boolean(false))], Some(&account))
            .is_err()
    );
    assert_eq!(sponsor.live_storage_cells(), 0);
}
