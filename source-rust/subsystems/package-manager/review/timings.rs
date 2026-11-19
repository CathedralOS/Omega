/*
    We emit stage start notes and completion durations (`stage=<name>
    micros=<elapsed>`) immediately on drop whenever `OMEGA_REVIEW_TIMINGS` is
    set or a compile/check requests `--timings` through `requested`. Emitting
    each stage immediately ensures that an interrupted or timed-out package
    command retains completed stage timings and identifies which stage was still
    running when the process terminated.
*/

use std::borrow::Cow;
use std::cell::Cell;
use std::time::Instant;

const TIMINGS_VARIABLE: &str = "OMEGA_REVIEW_TIMINGS";

thread_local! {
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static ACTIVE: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct StageTiming {
    name: Cow<'static, str>,
    started: Instant,
}

pub(crate) fn stage(name: &'static str) -> Option<StageTiming> {
    enabled().then(|| StageTiming::start(name))
}

pub(crate) fn subject_stage(name: impl FnOnce() -> String) -> Option<StageTiming> {
    enabled().then(|| StageTiming::start_owned(Cow::Owned(name())))
}

pub(crate) fn requested(requested: bool) -> Option<RequestedTimings> {
    requested.then(|| RequestedTimings {
        previous: REQUESTED.with(|flag| flag.replace(true)),
    })
}

pub(crate) fn note(line: impl FnOnce() -> String) {
    if enabled() {
        eprintln!("omega_review_timings note={}", line());
    }
}

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
    fn start(name: &'static str) -> Self {
        Self::start_owned(Cow::Borrowed(name))
    }

    fn start_owned(name: Cow<'static, str>) -> Self {
        ACTIVE.with(|active| active.set(active.get() + 1));
        eprintln!("omega_review_timings note=stage_started stage={name}");
        Self {
            name,
            started: Instant::now(),
        }
    }
}

impl Drop for StageTiming {
    fn drop(&mut self) {
        let microseconds = u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX);
        eprintln!(
            "omega_review_timings stage={} micros={microseconds}",
            self.name
        );
        let remaining = ACTIVE.with(|active| {
            let remaining = active.get().saturating_sub(1);
            active.set(remaining);
            remaining
        });
        if remaining == 0 {
            eprintln!("omega_review_timings run_micros={microseconds}");
        }
    }
}
