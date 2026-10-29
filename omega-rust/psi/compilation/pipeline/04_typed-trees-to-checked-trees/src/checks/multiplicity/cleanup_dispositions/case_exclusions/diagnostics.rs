//! Temporary profile-only pressure counters; no payloads or compiler decisions.
//! Relaxed process totals are not a coherent snapshot or retained-RSS estimate.
use super::CaseObservationTable;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub(super) enum Lookup {
    Absent,
    FirstEqual,
    Collision,
}

enum Counter {
    LookupAbsent,
    LookupFirstEqual,
    CollisionFallbacks,
    CollisionRosterRowsUpperBound,
    DuplicateInserts,
    NewInserts,
    InsertSharedDetachAttempts,
    RetainNoRemoval,
    RetainSharedRemovalAttempts,
    DetachObservationRows,
    DetachMapCapacityEntries,
}

static COUNTERS: [AtomicU64; 11] = [const { AtomicU64::new(0) }; 11];
static CALLBACKS: AtomicU64 = AtomicU64::new(0);

fn add(counter: Counter, amount: usize) {
    let amount = u64::try_from(amount).unwrap_or(u64::MAX);
    let _ = COUNTERS[counter as usize]
        .fetch_update(Relaxed, Relaxed, |old| Some(old.saturating_add(amount)));
}

fn detach_pressure(table: &CaseObservationTable) {
    // Attempt pressure, not completed allocations: an allocation can fail.
    add(Counter::DetachObservationRows, table.observations.len());
    add(
        Counter::DetachMapCapacityEntries,
        table.by_identity.capacity(),
    );
}

pub(super) fn lookup(kind: Lookup, roster_upper_bound: usize) {
    if !phase_profile::enabled() {
        return;
    }
    add(
        match kind {
            Lookup::Absent => Counter::LookupAbsent,
            Lookup::FirstEqual => Counter::LookupFirstEqual,
            Lookup::Collision => Counter::CollisionFallbacks,
        },
        1,
    );
    add(Counter::CollisionRosterRowsUpperBound, roster_upper_bound);
    progress();
}

pub(super) fn insert(duplicate: bool, table: &Arc<CaseObservationTable>) {
    if !phase_profile::enabled() {
        return;
    }
    if duplicate {
        add(Counter::DuplicateInserts, 1);
    } else {
        add(Counter::NewInserts, 1);
        if Arc::strong_count(table) > 1 {
            add(Counter::InsertSharedDetachAttempts, 1);
            detach_pressure(table);
        }
    }
    progress();
}

pub(super) fn retain(removed: bool, table: &Arc<CaseObservationTable>) {
    if !phase_profile::enabled() {
        return;
    }
    if !removed {
        add(Counter::RetainNoRemoval, 1);
    } else if Arc::strong_count(table) > 1 {
        add(Counter::RetainSharedRemovalAttempts, 1);
        detach_pressure(table);
    }
    progress();
}

fn progress() {
    let prior = CALLBACKS
        .fetch_update(Relaxed, Relaxed, |old| Some(old.saturating_add(1)))
        .unwrap_or_else(|old| old);
    let count = prior.saturating_add(1);
    if count < 1024 || !count.is_power_of_two() {
        return;
    }
    let values = COUNTERS.each_ref().map(|counter| counter.load(Relaxed));
    eprintln!(
        "case-observation-pressure callbacks={count} lookup_absent={} lookup_first_equal={} collision_fallbacks={} collision_roster_rows_upper_bound={} duplicate_inserts={} new_insert_attempts={} insert_shared_detach_attempts={} retain_no_removal={} retain_shared_removal_attempts={} detach_observation_rows={} detach_map_capacity_entries={}",
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        values[6],
        values[7],
        values[8],
        values[9],
        values[10]
    );
}
