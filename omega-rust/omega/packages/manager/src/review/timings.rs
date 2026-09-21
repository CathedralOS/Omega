//! Opt-in wall-clock attribution for the package-review route's stages.
//!
//! `wiki/drafts/test_cycle_measurements.md` attributes the checked-passes
//! route but leaves the manager-side review work between compilations
//! undivided (~250 s there). Setting `OMEGA_REVIEW_TIMINGS` prints one
//! `stage=<name> micros=<elapsed>` line per instrumented stage to stderr —
//! the same opt-in observation convention as `OMEGA_PROOF_MEASUREMENTS` —
//! when the outermost instrumented stage finishes, so an ordinary
//! `omega install|update|audit packages` supplies the figures a hotspot
//! study needs. Normal invocations collect nothing beyond one env-var
//! check per stage; measurements describe cost, never a verdict.
//!
//! The CLI already owns an opt-in `--timings` route for command-stage
//! durations (`omega-rust/omega/src/cli/arguments/compile.rs`); this
//! env-var channel is a second observation surface and should either be
//! reached from `--timings` on `install|update|audit packages` or folded
//! into it before it is documented as a user-facing knob.

use std::cell::{Cell, RefCell};
use std::time::Instant;

const TIMINGS_VARIABLE: &str = "OMEGA_REVIEW_TIMINGS";

thread_local! {
    /// Completed `(stage, elapsed µs)` pairs for the current run, in
    /// finishing order; the outermost stage completes last, so its elapsed
    /// is the run total.
    static COMPLETED: RefCell<Vec<(&'static str, u64)>> = const { RefCell::new(Vec::new()) };
    /// Live stage guards on this thread. A stage nested inside another
    /// (for example triage inside source-review assembly) reports inside
    /// the outermost stage's run instead of flushing alone.
    static ACTIVE: Cell<usize> = const { Cell::new(0) };
}

/// Measures one review stage from construction to drop.
pub(crate) struct StageTiming {
    name: &'static str,
    started: Instant,
}

/// Record stage `name` for this invocation when `OMEGA_REVIEW_TIMINGS` is
/// set; does nothing otherwise. Stages are named for the operation their
/// entry point performs (`candidate_compilation`, `policy_comparison`, ...).
pub(crate) fn stage(name: &'static str) -> Option<StageTiming> {
    if std::env::var_os(TIMINGS_VARIABLE).is_none() {
        return None;
    }
    Some(StageTiming::start(name))
}

impl StageTiming {
    /// `stage`'s unconditional half, split so tests exercise the ledger
    /// without mutating process environment.
    fn start(name: &'static str) -> Self {
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
            completed.borrow_mut().push((self.name, microseconds));
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
fn completed_stages() -> Vec<(&'static str, u64)> {
    COMPLETED.with(|completed| completed.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn sequential_stages_flush_independently() {
        drop(StageTiming::start("first"));
        assert!(completed_stages().is_empty());
        drop(StageTiming::start("second"));
        assert!(completed_stages().is_empty());
    }
}
