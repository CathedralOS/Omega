#![forbid(unsafe_code)]

//! Process-wide compile-time attribution scopes and event counters.
//!
//! Stage 05 used to be one opaque timer. These scopes let a measurement run
//! split it by pass without temporary, reverted instrumentation. Collection
//! is off unless `OMEGA_PHASE_PROFILE` is set when the process first asks;
//! a disabled scope costs one relaxed load and records nothing.
//!
//! Rows are keyed by `&'static str` names (dot-separated by convention, e.g.
//! `05.check.flow`). Times are inclusive wall time on the recording thread:
//! nested scopes overlap their parents, and concurrent compilations in one
//! process add together. This is measurement, never semantic input: no
//! compiler decision may read these values.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

const UNKNOWN: u8 = 0;
const DISABLED: u8 = 1;
const ENABLED: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(UNKNOWN);
static ROWS: Mutex<Vec<Row>> = Mutex::new(Vec::new());

/// One accumulated scope or counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub name: &'static str,
    /// Scope entries, or the summed amount for a counter.
    pub count: u64,
    /// Inclusive nanoseconds; zero for a pure counter.
    pub nanos: u64,
}

/// Whether collection is active for this process.
#[inline]
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ENABLED => true,
        DISABLED => false,
        _ => initialize(),
    }
}

#[cold]
fn initialize() -> bool {
    let on = std::env::var_os("OMEGA_PHASE_PROFILE").is_some_and(|value| value != "0");
    STATE.store(if on { ENABLED } else { DISABLED }, Ordering::Relaxed);
    on
}

/// Time `work` under `name` when collection is enabled.
#[inline]
pub fn scope<T>(name: &'static str, work: impl FnOnce() -> T) -> T {
    if !enabled() {
        return work();
    }
    let started = Instant::now();
    let result = work();
    add(name, 1, elapsed_nanos(started));
    result
}

/// A guard form for regions that are awkward to wrap in a closure.
#[must_use = "a scope guard records when it is dropped"]
pub struct Guard {
    name: &'static str,
    started: Option<Instant>,
}

#[inline]
pub fn guard(name: &'static str) -> Guard {
    Guard {
        name,
        started: enabled().then(Instant::now),
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            add(self.name, 1, elapsed_nanos(started));
        }
    }
}

/// Add `amount` events to a counter.
#[inline]
pub fn count(name: &'static str, amount: u64) {
    if enabled() {
        add(name, amount, 0);
    }
}

fn elapsed_nanos(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn add(name: &'static str, count: u64, nanos: u64) {
    let mut rows = ROWS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(row) = rows
        .iter_mut()
        .find(|row| std::ptr::eq(row.name, name) || row.name == name)
    {
        row.count = row.count.saturating_add(count);
        row.nanos = row.nanos.saturating_add(nanos);
    } else {
        rows.push(Row { name, count, nanos });
    }
}

/// Snapshot of every row, sorted by name so nested names group together.
pub fn snapshot() -> Vec<Row> {
    let mut rows = ROWS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    rows.sort_by(|left, right| left.name.cmp(right.name));
    rows
}

/// Render the snapshot as aligned text, one row per line.
pub fn render() -> String {
    let rows = snapshot();
    let mut text = String::new();
    for row in rows {
        if row.nanos == 0 {
            text.push_str(&format!("{:>12} n  {}\n", row.count, row.name));
        } else {
            text.push_str(&format!(
                "{:>10.3} ms  {:>8} x  {}\n",
                row.nanos as f64 / 1_000_000.0,
                row.count,
                row.name
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn rows_accumulate_by_name() {
        super::STATE.store(super::ENABLED, std::sync::atomic::Ordering::Relaxed);
        super::count("test.counter", 2);
        super::count("test.counter", 3);
        super::scope("test.scope", || ());
        let rows = super::snapshot();
        let counter = rows.iter().find(|row| row.name == "test.counter").unwrap();
        assert_eq!(counter.count, 5);
        assert_eq!(counter.nanos, 0);
        assert_eq!(
            rows.iter()
                .find(|row| row.name == "test.scope")
                .unwrap()
                .count,
            1
        );
    }
}
