#![forbid(unsafe_code)]

//! Process-wide compile-time attribution scopes and event counters.
//!
//! These scopes let a measurement run split a compile by pass without
//! temporary, reverted instrumentation. Collection is off unless
//! `OMEGA_PHASE_PROFILE` is set when the process first asks; a disabled scope
//! costs one relaxed load and records nothing.
//!
//! Rows are keyed by `&'static str` names. A name starts with `psi.` or
//! `omega.`. A scope that covers exactly one pipeline stage continues with that
//! stage's folder number (`psi.05` is typed-to-checked trees, `omega.05`
//! register homes); work outside a single stage uses a word (`psi.source`,
//! `omega.product`, `omega.native`). Nested scopes append dot-separated words,
//! so a child sorts under its parent: `psi.05.check_checked_facts` contains
//! `psi.05.check_checked_facts.check_linear_obligations`. Names carry no
//! outline numbers or letters.
//!
//! Times are inclusive wall time on the recording thread: nested scopes
//! overlap their parents, and concurrent compilations in one process add
//! together. This is measurement, never semantic input: no
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

/// One pass's inclusive time on one subject, such as a single machine.
/// `subject` is `file\tdeclaration`; the caller names it only while enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectRow {
    pub pass: &'static str,
    pub subject: String,
    pub count: u64,
    pub nanos: u64,
}

static SUBJECT_ROWS: Mutex<Vec<SubjectRow>> = Mutex::new(Vec::new());
// Subjects that began while a subject of the same pass was open on the same
// thread: their time is counted twice. Parallel workers never nest here.
static NESTED: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());

thread_local! {
    static OPEN_PASSES: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Attribute the guarded work of `pass` to one subject. Subjects of one pass
/// should not nest, or their sum exceeds the pass; [`render_subjects`]
/// reports that sum beside the pass's own row so overlap and gaps show.
#[must_use = "a subject guard records when it is dropped"]
pub struct SubjectGuard {
    pass: &'static str,
    subject: Option<(String, Instant)>,
}

#[inline]
pub fn subject(pass: &'static str, name: impl FnOnce() -> String) -> SubjectGuard {
    if !enabled() {
        return SubjectGuard { pass, subject: None };
    }
    let nested = OPEN_PASSES.with(|open| {
        let mut open = open.borrow_mut();
        let nested = open.contains(&pass);
        open.push(pass);
        nested
    });
    if nested {
        let mut rows = NESTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match rows.iter_mut().find(|(name, _)| *name == pass) {
            Some(row) => row.1 += 1,
            None => rows.push((pass, 1)),
        }
    }
    SubjectGuard {
        pass,
        subject: Some((name(), Instant::now())),
    }
}

impl Drop for SubjectGuard {
    fn drop(&mut self) {
        let Some((subject, started)) = self.subject.take() else {
            return;
        };
        let nanos = elapsed_nanos(started);
        OPEN_PASSES.with(|open| {
            let mut open = open.borrow_mut();
            if let Some(position) = open.iter().rposition(|pass| *pass == self.pass) {
                open.remove(position);
            }
        });
        let mut rows = SUBJECT_ROWS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match rows.binary_search_by(|row| {
            (row.pass, row.subject.as_str()).cmp(&(self.pass, subject.as_str()))
        }) {
            Ok(position) => {
                let row = &mut rows[position];
                row.count = row.count.saturating_add(1);
                row.nanos = row.nanos.saturating_add(nanos);
            }
            Err(position) => rows.insert(
                position,
                SubjectRow {
                    pass: self.pass,
                    subject,
                    count: 1,
                    nanos,
                },
            ),
        }
    }
}

pub fn subject_snapshot() -> Vec<SubjectRow> {
    SUBJECT_ROWS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Per pass: how much of the pass's own row its subjects explain, the top
/// `limit` subjects, and the same time grouped by source file. A coverage
/// far below 100% means unattributed work; above 100% means subjects nest or
/// ran in parallel, so their wall times overlap.
pub fn render_subjects(limit: usize) -> String {
    let rows = subject_snapshot();
    let passes = snapshot();
    let mut text = String::new();
    let mut start = 0;
    while start < rows.len() {
        let pass = rows[start].pass;
        let end = start + rows[start..].iter().take_while(|row| row.pass == pass).count();
        let group = &rows[start..end];
        let attributed: u64 = group.iter().map(|row| row.nanos).sum();
        let own = passes
            .iter()
            .find(|row| row.name == pass)
            .map(|row| row.nanos);
        let ms = |nanos: u64| nanos as f64 / 1_000_000.0;
        text.push_str(&format!(
            "SUBJECTS {pass}: {} subject(s), {:.3} ms attributed",
            group.len(),
            ms(attributed)
        ));
        match own {
            Some(own) if own > 0 => text.push_str(&format!(
                " of {:.3} ms pass ({:.0}% coverage",
                ms(own),
                attributed as f64 * 100.0 / own as f64
            )),
            _ => text.push_str(" (pass has no own row"),
        }
        let nested = NESTED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|(name, _)| *name == pass)
            .map_or(0, |(_, count)| *count);
        // Over 100% with no nesting is parallel workers; nesting is double count.
        text.push_str(&format!(", {nested} nested)\n"));
        let mut by_subject: Vec<&SubjectRow> = group.iter().collect();
        by_subject.sort_by(|left, right| right.nanos.cmp(&left.nanos));
        for row in by_subject.iter().take(limit) {
            text.push_str(&format!(
                "  {:>10.3} ms {:>6} x  {}\n",
                ms(row.nanos),
                row.count,
                row.subject.replace('\t', "  ")
            ));
        }
        let mut by_file: Vec<(&str, u64, u64)> = Vec::new();
        for row in group {
            let file = row.subject.split('\t').next().unwrap_or_default();
            match by_file.iter_mut().find(|(name, ..)| *name == file) {
                Some(entry) => {
                    entry.1 += row.nanos;
                    entry.2 += 1;
                }
                None => by_file.push((file, row.nanos, 1)),
            }
        }
        by_file.sort_by(|left, right| right.1.cmp(&left.1));
        text.push_str("  by file:\n");
        for (file, nanos, subjects) in by_file.iter().take(limit) {
            text.push_str(&format!(
                "  {:>10.3} ms {:>6} subj  {file}\n",
                ms(*nanos),
                subjects
            ));
        }
        start = end;
    }
    text
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

    #[test]
    fn subjects_accumulate_and_report_coverage_against_their_pass() {
        super::STATE.store(super::ENABLED, std::sync::atomic::Ordering::Relaxed);
        super::scope("test.pass", || {
            for _ in 0..2 {
                let _a = super::subject("test.pass", || "a.omg	A::run".to_owned());
            }
            let _b = super::subject("test.pass", || "b.omg	B::run".to_owned());
        });
        let rows: Vec<_> = super::subject_snapshot()
            .into_iter()
            .filter(|row| row.pass == "test.pass")
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].subject, "a.omg	A::run");
        assert_eq!(rows[0].count, 2);
        let report = super::render_subjects(5);
        assert!(report.contains("SUBJECTS test.pass: 2 subject(s)"), "{report}");
        assert!(report.contains("% coverage"), "{report}");
        assert!(report.contains("a.omg"), "{report}");
    }

    #[test]
    fn a_subject_opened_inside_the_same_pass_counts_as_nested() {
        super::STATE.store(super::ENABLED, std::sync::atomic::Ordering::Relaxed);
        super::scope("test.nested", || {
            let _outer = super::subject("test.nested", || "x.omg	Outer".to_owned());
            let _inner = super::subject("test.nested", || "x.omg	Inner".to_owned());
        });
        let _sibling = super::subject("test.nested", || "x.omg	After".to_owned());
        let report = super::render_subjects(5);
        assert!(report.contains(", 1 nested)"), "{report}");
    }
}
