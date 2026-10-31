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
//! Keep rows sorted as names first appear: frequent updates then search only
//! logarithmically. A linear name scan charged profiler bookkeeping to parent
//! scopes, obscuring the cost of small, frequently observed compiler operations.

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
    match rows.binary_search_by(|row| row.name.cmp(name)) {
        Ok(position) => {
            let row = &mut rows[position];
            row.count = row.count.saturating_add(count);
            row.nanos = row.nanos.saturating_add(nanos);
        }
        Err(position) => rows.insert(position, Row { name, count, nanos }),
    }
}

/// Snapshot of every row, sorted by name so nested names group together.
pub fn snapshot() -> Vec<Row> {
    ROWS.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Render the snapshot as aligned text, one row per line.
pub fn render() -> String {
    render_rows(snapshot())
}

fn render_rows(rows: impl IntoIterator<Item = Row>) -> String {
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

/// Emit one coarse boundary and one completed scope/counter row. Names are
/// exact: this never streams the per-node scope roster. Values are inclusive
/// wall-time totals across this process, including overlapping compilations;
/// they are not exclusive CPU time or an active scope's elapsed time.
/// Reporting cannot change compiler errors when stderr is unavailable.
pub fn checkpoint(label: &'static str, completed_scope: &'static str) {
    if !enabled() {
        return;
    }
    let row = {
        let rows = ROWS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        rows.binary_search_by(|row| row.name.cmp(completed_scope))
            .ok()
            .map(|position| rows[position])
    };
    // Copy the single row before formatting or I/O; no semantic work or
    // diagnostic output runs while the profiler mutex is held.
    let mut text = format!(
        "CHECKPOINT {label} pid={} thread={:?} process-cumulative completed inclusive-wall\n",
        std::process::id(),
        std::thread::current().id(),
    );
    text.push_str(&render_rows(row));
    use std::io::Write;
    let _ = std::io::stderr().lock().write_all(text.as_bytes());
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

        // Names are content identities even when separate callers own them.
        let first: &'static str = Box::leak(String::from("test.equal").into_boxed_str());
        let second: &'static str = Box::leak(String::from("test.equal").into_boxed_str());
        assert!(!std::ptr::eq(first, second));
        super::count(first, 7);
        super::count(second, 11);
        super::add("test.saturated", u64::MAX, u64::MAX);
        super::add("test.saturated", 1, 1);
        let workers = (0..4)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..1000 {
                        super::count("test.concurrent", 1);
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let rows = super::snapshot();
        assert!(rows.windows(2).all(|pair| pair[0].name < pair[1].name));
        assert_eq!(rows.iter().find(|row| row.name == first).unwrap().count, 18);
        assert_eq!(
            rows.iter()
                .find(|row| row.name == "test.concurrent")
                .unwrap()
                .count,
            4000
        );
        let saturated = rows
            .iter()
            .find(|row| row.name == "test.saturated")
            .unwrap();
        assert_eq!(saturated.count, u64::MAX);
        assert_eq!(saturated.nanos, u64::MAX);
    }
}
