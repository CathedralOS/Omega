//! Source-free accounting falsifiers. These exercise counters/ownership only;
//! no test here claims that an interpreter charges its actual storage sites.

use evaluation_accounting::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits, BuildEvaluationUsageError,
    EvaluationAccountingError, EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;
use std::sync::{Arc, Barrier};

fn limits(fuel: u64, cells: u64, bytes: u64) -> BuildEvaluationSponsorLimits {
    BuildEvaluationSponsorLimits::new(fuel, 8, 2, 2, cells, bytes, cells, bytes).unwrap()
}

fn terminal() -> EvaluationMeasurementIdentity {
    EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
    }
}

fn sponsor(cells: u64, bytes: u64) -> BuildEvaluationSponsor {
    BuildEvaluationSponsor::new_with_measurement(limits(10, cells, bytes), terminal()).unwrap()
}

#[test]
fn aggregate_identity_refuses_both_engine_substitutions_without_charge() {
    let tree = BuildEvaluationSponsor::new(limits(10, 4, 8));
    tree.charge_fuel_unit().unwrap();
    assert!(tree.begin_invocation(terminal(), 10).is_err());
    assert_eq!(tree.consumed_fuel_units(), 1);
    let terminal = sponsor(4, 8);
    let invocation = terminal
        .begin_invocation(terminal.measurement(), 10)
        .unwrap();
    invocation.charge_fuel(1).unwrap();
    assert!(
        terminal
            .begin_invocation(EvaluationMeasurementIdentity::CheckedTree, 10)
            .is_err()
    );
    assert!(terminal.charge_fuel_unit().is_err());
    assert!(terminal.reserve_live_cell().is_err());
    assert!(terminal.reserve_live_text_bytes(1).is_err());
    assert!(terminal.charge_result_custody(1, 1).is_err());
    assert_eq!(terminal.consumed_fuel_units(), 1);
    assert_eq!(terminal.live_storage_cells(), 0);
    assert_eq!(terminal.live_payload_bytes(), 0);
    assert_eq!(terminal.consumed_result_storage_cells(), 0);
}

#[test]
fn rejected_combined_site_changes_neither_fuel_nor_storage() {
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 5).unwrap();
    let before = *invocation.snapshot().unwrap().record();
    assert!(invocation.reserve_site(3, 4, 9).is_err());
    assert_eq!(*invocation.snapshot().unwrap().record(), before);
    assert_eq!(sponsor.consumed_fuel_units(), 0);
    let lease = invocation.reserve_site(3, 4, 8).unwrap();
    assert!(invocation.reserve_site(3, 0, 0).is_err());
    assert_eq!(sponsor.consumed_fuel_units(), 3);
    drop(lease);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 0);
    assert_eq!(sponsor.consumed_fuel_units(), 3);
}

#[test]
fn two_invocations_contend_atomically_for_one_live_allowance() {
    let sponsor = sponsor(4, 8);
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let lease = invocation.reserve_storage(3, 6);
                barrier.wait();
                let record = *invocation.snapshot().unwrap().record();
                barrier.wait();
                (lease.is_ok(), record)
            })
        })
        .collect();
    barrier.wait();
    barrier.wait();
    assert_eq!(sponsor.live_storage_cells(), 3);
    assert_eq!(sponsor.live_payload_bytes(), 6);
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|(reserved, _)| *reserved).count(), 1);
    let (_, rejected) = results.iter().find(|(reserved, _)| !reserved).unwrap();
    assert_eq!(rejected.peak_live_storage_cells, 0);
    assert_eq!(rejected.peak_live_payload_bytes, 0);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}

#[test]
fn moving_and_arc_aliasing_releases_only_the_final_lease() {
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
    let lease = invocation.reserve_storage(2, 4).unwrap();
    let moved = Arc::new(lease);
    let alias = Arc::clone(&moved);
    drop(moved);
    assert_eq!(sponsor.live_payload_bytes(), 4);
    drop(alias);
    assert_eq!(sponsor.live_payload_bytes(), 0);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn growth_refusal_and_excess_shrink_preserve_held_storage() {
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
    let mut lease = invocation.reserve_storage(2, 4).unwrap();
    let before = *invocation.snapshot().unwrap().record();
    assert!(lease.grow(3, 1).is_err());
    assert!(lease.shrink(3, 1).is_err());
    assert_eq!(*invocation.snapshot().unwrap().record(), before);
    lease.grow(2, 4).unwrap();
    lease.shrink(1, 2).unwrap();
    assert_eq!(sponsor.live_storage_cells(), 3);
    assert_eq!(sponsor.live_payload_bytes(), 6);
    drop(lease);
    assert_eq!(sponsor.live_storage_cells(), 0);
}

#[test]
fn joining_leases_transfers_both_dimensions_without_recharging() {
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
    let mut left = invocation.reserve_storage(2, 3).unwrap();
    let mut right = invocation.reserve_storage(2, 5).unwrap();
    let before = *invocation.snapshot().unwrap().record();
    left.absorb(&mut right).unwrap();
    drop(right);
    assert_eq!(*invocation.snapshot().unwrap().record(), before);
    assert_eq!(sponsor.live_storage_cells(), 4);
    assert_eq!(sponsor.live_payload_bytes(), 8);
    left.truncate_cells(u64::MAX);
    assert_eq!(sponsor.live_storage_cells(), 4);
    left.truncate_cells(1);
    assert_eq!(sponsor.live_storage_cells(), 1);
    assert_eq!(sponsor.live_payload_bytes(), 8);
    drop(left);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}

#[test]
fn joining_sibling_leases_refuses_without_moving_either_reservation() {
    let sponsor = sponsor(4, 8);
    let first = sponsor.begin_invocation(terminal(), 10).unwrap();
    let second = sponsor.begin_invocation(terminal(), 10).unwrap();
    let mut left = first.reserve_storage(1, 3).unwrap();
    let mut right = second.reserve_storage(2, 4).unwrap();
    assert_eq!(
        left.absorb(&mut right),
        Err(EvaluationAccountingError::LeaseAccountMismatch)
    );
    assert_eq!(sponsor.live_storage_cells(), 3);
    assert_eq!(sponsor.live_payload_bytes(), 7);
    drop(right);
    assert_eq!(sponsor.live_storage_cells(), 1);
    assert_eq!(sponsor.live_payload_bytes(), 3);
    drop(left);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.live_payload_bytes(), 0);
}

#[test]
fn equal_limits_do_not_transfer_usage_authority() {
    let first = sponsor(4, 8);
    let second = sponsor(4, 8);
    let invocation = first.begin_invocation(terminal(), 10).unwrap();
    let sibling = first.begin_invocation(terminal(), 10).unwrap();
    let usage = invocation.snapshot().unwrap();
    assert!(first.owns_usage(&usage));
    assert!(first.clone().owns_usage(&usage));
    assert!(!second.owns_usage(&usage));
    assert!(invocation.clone().owns_usage(&usage));
    assert!(!sibling.owns_usage(&usage));
}

#[test]
fn retained_result_storage_and_custody_span_retries_without_refund() {
    let sponsor = sponsor(4, 8);
    let first = sponsor.begin_invocation(terminal(), 10).unwrap();
    let retained_result = first.reserve_storage(2, 4).unwrap();
    first.charge_result_custody(2, 4).unwrap();
    let retry = sponsor.begin_invocation(terminal(), 10).unwrap();
    assert!(retry.reserve_storage(3, 4).is_err());
    retry.charge_result_custody(2, 4).unwrap();
    assert!(retry.charge_result_custody(1, 0).is_err());
    assert_eq!(sponsor.consumed_result_storage_cells(), 4);
    drop(retained_result);
    assert_eq!(sponsor.live_storage_cells(), 0);
    assert_eq!(sponsor.consumed_result_payload_bytes(), 8);
    assert!(retry.charge_result_custody(0, 1).is_err());
}

#[test]
fn host_charges_are_observed_locally_and_aggregate_refusal_is_atomic() {
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
    invocation.charge_build_log_bytes(8).unwrap();
    invocation.charge_filesystem_operation_attempt().unwrap();
    invocation.charge_filesystem_operation_attempt().unwrap();
    let before = *invocation.snapshot().unwrap().record();
    assert!(invocation.charge_build_log_bytes(1).is_err());
    assert!(invocation.charge_filesystem_operation_attempt().is_err());
    assert_eq!(*invocation.snapshot().unwrap().record(), before);
    assert_eq!(before.build_log_bytes, 8);
    assert_eq!(before.filesystem_operation_attempts, 2);
}

#[test]
fn unsupported_storage_schedule_is_not_schedule_one_by_default() {
    let identity = EvaluationMeasurementIdentity::TerminalPsi {
        fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
        storage_schedule: TerminalStorageScheduleIdentity::new(2).unwrap(),
    };
    assert_ne!(EvaluationMeasurementIdentity::CheckedTree, terminal());
    assert!(BuildEvaluationSponsor::new_with_measurement(limits(10, 4, 8), identity).is_err());
    let sponsor = sponsor(4, 8);
    let invocation = sponsor.begin_invocation(terminal(), 10).unwrap();
    let mut usage = *invocation.snapshot().unwrap().record();
    usage.measurement = identity;
    assert_eq!(
        usage.validate(),
        Err(BuildEvaluationUsageError::UnsupportedMeasurementIdentity)
    );
    usage.measurement = EvaluationMeasurementIdentity::CheckedTree;
    assert_eq!(
        usage.validate(),
        Err(BuildEvaluationUsageError::UnsupportedMeasurementIdentity)
    );
}

#[test]
fn fuel_reservation_overflow_preserves_both_accounts() {
    let sponsor =
        BuildEvaluationSponsor::new_with_measurement(limits(u64::MAX, 4, 8), terminal()).unwrap();
    let invocation = sponsor.begin_invocation(terminal(), u64::MAX).unwrap();
    invocation.charge_fuel(u64::MAX).unwrap();
    let before = *invocation.snapshot().unwrap().record();
    assert!(invocation.charge_fuel(1).is_err());
    assert!(invocation.reserve_site(1, 1, 1).is_err());
    assert_eq!(*invocation.snapshot().unwrap().record(), before);
}
