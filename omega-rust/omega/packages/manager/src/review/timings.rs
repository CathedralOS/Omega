//! Opt-in wall-clock attribution for the package-review route's stages.
//!
//! `wiki/drafts/measurements/test_cycle_measurements.md` attributes the checked-passes
//! route but leaves the manager-side review work between compilations
//! undivided (~250 s there). Setting `OMEGA_REVIEW_TIMINGS` prints one
//! `stage=<name> micros=<elapsed>` line per instrumented stage to stderr —
//! the same opt-in observation convention as `OMEGA_PROOF_MEASUREMENTS` —
//! when the outermost instrumented stage finishes, so an ordinary
//! `omega install|update|audit packages` supplies the figures a hotspot
//! study needs. Normal invocations collect nothing beyond one env-var
//! check per stage; measurements describe cost, never a verdict.
//!
//! A compile or check that asked for `--timings` requests the same stages
//! through [`requested`] for the duration of its candidate compilation, so
//! the per-pass and per-package lines print beside the command's stage
//! ladder without the variable. `install|update|audit packages` still reach
//! them only through the variable.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::time::Instant;

const TIMINGS_VARIABLE: &str = "OMEGA_REVIEW_TIMINGS";

thread_local! {
    /// Completed `(stage, elapsed µs)` pairs for the current run, in
    /// finishing order; the outermost stage completes last, so its elapsed
    /// is the run total.
    static COMPLETED: RefCell<Vec<(Cow<'static, str>, u64)>> = const { RefCell::new(Vec::new()) };
    /// A caller on this thread asked for review timings for a bounded scope.
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    /// Live stage guards on this thread. A stage nested inside another
    /// (for example triage inside source-review assembly) reports inside
    /// the outermost stage's run instead of flushing alone.
    static ACTIVE: Cell<usize> = const { Cell::new(0) };
}

/// Measures one review stage from construction to drop.
pub(crate) struct StageTiming {
    name: Cow<'static, str>,
    started: Instant,
}

/// Record stage `name` for this invocation when `OMEGA_REVIEW_TIMINGS` is
/// set; does nothing otherwise. Stages are named for the operation their
/// entry point performs (`candidate_compilation`, `policy_comparison`, ...).
pub(crate) fn stage(name: &'static str) -> Option<StageTiming> {
    enabled().then(|| StageTiming::start(name))
}

/// The same measurement for a stage whose name carries its subject -- the
/// review pass, or one package occurrence inside it. The name is built only
/// when the variable is set, so an ordinary run still pays one env-var check
/// and no formatting.
pub(crate) fn subject_stage(name: impl FnOnce() -> String) -> Option<StageTiming> {
    enabled().then(|| StageTiming::start_owned(Cow::Owned(name())))
}

/// Record review stages on this thread while the returned guard lives when
/// `requested` is true, as the variable does for a whole run. Dropping the
/// guard restores the previous request, so nested scopes compose.
pub(crate) fn requested(requested: bool) -> Option<RequestedTimings> {
    requested.then(|| RequestedTimings {
        previous: REQUESTED.with(|flag| flag.replace(true)),
    })
}

/// A scope in which a caller asked for review timings.
pub(crate) struct RequestedTimings {
    previous: bool,
}

impl Drop for RequestedTimings {
    fn drop(&mut self) {
        REQUESTED.with(|flag| flag.set(self.previous));
    }
}

fn enabled() -> bool {
    REQUESTED.with(Cell::get) || std::env::var_os(TIMINGS_VARIABLE).is_some()
}

impl StageTiming {
    /// `stage`'s unconditional half, split so tests exercise the ledger
    /// without mutating process environment.
    fn start(name: &'static str) -> Self {
        Self::start_owned(Cow::Borrowed(name))
    }

    fn start_owned(name: Cow<'static, str>) -> Self {
        ACTIVE.with(|active| active.set(active.get() + 1));
        Self {
            name,
            started: Instant::now(),
        }
    }
}

impl Drop for StageTiming {
    fn drop(&mut self) {
        let microseconds = u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        COMPLETED.with(|completed| {
            completed
                .borrow_mut()
                .push((std::mem::take(&mut self.name), microseconds));
        });
        let remaining = ACTIVE.with(|active| {
            let remaining = active.get().saturating_sub(1);
            active.set(remaining);
            remaining
        });
        if remaining == 0 {
            COMPLETED.with(|completed| {
                for (stage, elapsed) in completed.borrow_mut().drain(..) {
                    eprintln!("omega_review_timings stage={stage} micros={elapsed}");
                }
            });
            eprintln!("omega_review_timings run_micros={microseconds}");
        }
    }
}

#[cfg(test)]
fn completed_stages() -> Vec<(Cow<'static, str>, u64)> {
    COMPLETED.with(|completed| completed.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::{StageTiming, completed_stages};

    #[test]
    fn nested_stages_report_inside_the_outermost_run() {
        let outer = StageTiming::start("outer");
        {
            let inner = StageTiming::start("inner");
            drop(inner);
            let recorded = completed_stages();
            assert_eq!(recorded.len(), 1);
            assert_eq!(recorded[0].0, "inner");
        }
        drop(outer);
        assert!(completed_stages().is_empty());
    }

    #[test]
    fn a_requested_scope_records_stages_without_the_variable() {
        let variable_set = std::env::var_os(super::TIMINGS_VARIABLE).is_some();
        {
            let _requested = super::requested(true);
            let stage = super::stage("requested");
            assert!(stage.is_some());
            drop(stage);
        }
        if !variable_set {
            assert!(super::stage("unrequested").is_none());
            assert!(super::requested(false).is_none());
        }
    }

    #[test]
    fn sequential_stages_flush_independently() {
        drop(StageTiming::start("first"));
        assert!(completed_stages().is_empty());
        drop(StageTiming::start("second"));
        assert!(completed_stages().is_empty());
    }
}
