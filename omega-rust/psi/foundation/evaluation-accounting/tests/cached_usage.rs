//! Recorded-cost replay tests; public Build execution is covered by build_charge.
use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildEvaluationUsage,
    EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;
use std::sync::{Arc, Barrier};

fn limits(
    fuel: u64,
    log: u64,
    cells: u64,
    bytes: u64,
    result_cells: u64,
    result_bytes: u64,
) -> BuildEvaluationSponsorLimits {
    BuildEvaluationSponsorLimits::new(fuel, log, 8, 8, cells, bytes, result_cells, result_bytes)
        .unwrap()
}
fn measured() -> BuildEvaluationUsage {
    let sponsor = BuildEvaluationSponsor::new(limits(100, 100, 100, 100, 100, 100));
    let invocation = sponsor
        .begin_invocation(EvaluationMeasurementIdentity::CheckedTree, 100)
        .unwrap();
    let storage = invocation.reserve_site(7, 3, 6).unwrap();
    invocation.charge_build_log_bytes(2).unwrap();
    invocation.charge_result_custody(2, 4).unwrap();
    drop(storage);
    *invocation.snapshot().unwrap().record()
}
#[test]
fn replay_spends_current_account_and_refreshes_its_peaks_without_live_leases() {
    let usage = measured();
    let sponsor = BuildEvaluationSponsor::new(limits(100, 100, 100, 100, 100, 100));
    let existing = sponsor.reserve_live_text_bytes(11).unwrap();
    let current = sponsor.charge_pure_usage(usage).unwrap();
    assert_eq!(sponsor.consumed_fuel_units(), 7);
    assert_eq!(sponsor.consumed_result_storage_cells(), 2);
    assert_eq!(sponsor.consumed_result_payload_bytes(), 4);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 11);
    assert_eq!(current.session_peak_live_payload_bytes, 17);
    assert_eq!(
        current.peak_live_payload_bytes,
        usage.peak_live_payload_bytes
    );
    assert_eq!(usage.session_peak_live_payload_bytes, 6);
    drop(existing);
    sponsor.charge_pure_usage(usage).unwrap();
    assert_eq!(sponsor.consumed_fuel_units(), 14);
    assert_eq!(sponsor.consumed_result_storage_cells(), 4);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}
#[test]
fn every_refused_axis_leaves_all_counters_unchanged() {
    for limits in [
        limits(6, 100, 100, 100, 100, 100),
        limits(100, 1, 100, 100, 100, 100),
        limits(100, 100, 2, 100, 100, 100),
        limits(100, 100, 100, 5, 100, 100),
        limits(100, 100, 100, 100, 1, 100),
        limits(100, 100, 100, 100, 100, 3),
    ] {
        let sponsor = BuildEvaluationSponsor::new(limits);
        let invocation = sponsor
            .begin_invocation(EvaluationMeasurementIdentity::CheckedTree, 100)
            .unwrap();
        let before = *invocation.snapshot().unwrap().record();
        assert!(sponsor.charge_pure_usage(measured()).is_err());
        assert_eq!(*invocation.snapshot().unwrap().record(), before);
        assert_eq!(sponsor.consumed_fuel_units(), 0);
        assert_eq!(sponsor.consumed_result_storage_cells(), 0);
        assert_eq!(sponsor.consumed_result_payload_bytes(), 0);
    }
}
#[test]
fn invalid_records_effectful_usage_and_other_measurements_cannot_spend() {
    let sponsor = BuildEvaluationSponsor::new(limits(100, 100, 100, 100, 100, 100));
    let mut invalid = measured();
    invalid.step_schedule_marker = 0;
    assert!(sponsor.charge_pure_usage(invalid).is_err());
    let mut effects = measured();
    effects.filesystem_operation_attempts = 1;
    assert!(sponsor.charge_pure_usage(effects).is_err());
    let mut terminal = measured();
    terminal.measurement = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    };
    terminal.usage_schema_version = 8;
    terminal.validate().unwrap();
    assert!(sponsor.charge_pure_usage(terminal).is_err());
    assert_eq!(sponsor.consumed_fuel_units(), 0);
}
#[test]
fn replay_accounts_for_current_live_reservations() {
    let sponsor = BuildEvaluationSponsor::new(limits(100, 100, 3, 6, 100, 100));
    let held = sponsor.reserve_live_cell().unwrap();
    assert!(sponsor.charge_pure_usage(measured()).is_err());
    assert_eq!(sponsor.live_storage_cells(), 1);
    assert_eq!(sponsor.consumed_fuel_units(), 0);
    drop(held);
    sponsor.charge_pure_usage(measured()).unwrap();
    assert_eq!(sponsor.live_storage_cells(), 0);
}
#[test]
fn overflowing_replay_does_not_change_other_axes() {
    let sponsor = BuildEvaluationSponsor::new(limits(u64::MAX, 100, 100, 100, 100, 100));
    let invocation = sponsor
        .begin_invocation(EvaluationMeasurementIdentity::CheckedTree, u64::MAX)
        .unwrap();
    invocation.charge_fuel(u64::MAX).unwrap();
    assert!(
        sponsor
            .charge_pure_usage(measured())
            .unwrap_err()
            .contains("overflowed")
    );
    assert_eq!(sponsor.consumed_fuel_units(), u64::MAX);
    assert_eq!(sponsor.consumed_result_storage_cells(), 0);
}
#[test]
fn concurrent_replays_cannot_both_spend_one_allowance() {
    let sponsor = BuildEvaluationSponsor::new(limits(7, 100, 100, 100, 100, 100));
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let sponsor = sponsor.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                sponsor.charge_pure_usage(measured()).is_ok()
            })
        })
        .collect();
    barrier.wait();
    let successes = threads
        .into_iter()
        .map(|thread| usize::from(thread.join().unwrap()))
        .sum::<usize>();
    assert_eq!(successes, 1);
    assert_eq!(sponsor.consumed_fuel_units(), 7);
    assert_eq!(sponsor.consumed_result_storage_cells(), 2);
}
