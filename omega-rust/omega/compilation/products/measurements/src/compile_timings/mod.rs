// Phase-timing ladder (`StageMeta`, `PhaseTiming`, `CompileTimings`).
//
// `CompileTimings::is_enabled` is propagated into child collectors so disabled
// compilations skip inner clock reads instead of measuring and discarding rows
// at merge time. Repeated measurements for the same stage label accumulate in
// place without reordering first-seen phases.
//
// `BUILD_AND_CHECKED_CONTINUATION` spans build evaluation and the checked
// continuation it drives (including Psi 00–03 and pre-build checks), while
// `SOURCE_ASSEMBLY` spans package closure resolution and per-target source
// assembly before build evaluation.

pub mod counters;

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageMeta {
    pub name: &'static str,
    pub input: &'static str,
    pub output: &'static str,
}

impl StageMeta {
    pub const fn new(name: &'static str, input: &'static str, output: &'static str) -> Self {
        Self {
            name,
            input,
            output,
        }
    }

    pub fn label(self) -> String {
        format!("{}: {} -> {}", self.name, self.input, self.output)
    }
}

pub const SOURCE_FILES_TO_TOKENS: StageMeta =
    StageMeta::new("Psi 00", "SourceFiles", "TokenStreams");

pub const TOKENS_TO_SYNTAX_TREES: StageMeta =
    StageMeta::new("Psi 01", "TokenStreams", "SyntaxTrees");

pub const BUILD_AND_CHECKED_CONTINUATION: StageMeta = StageMeta::new(
    "Step: build",
    "AssembledSyntax",
    "TypedTrees (contains Psi 00-03)",
);

pub const SOURCE_ASSEMBLY: StageMeta =
    StageMeta::new("Step: assemble", "PreparedSources", "AssembledSyntax");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompileTimings {
    enabled: bool,
    phases: Vec<PhaseTiming>,
}

impl CompileTimings {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            phases: Vec::new(),
        }
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn phases(&self) -> &[PhaseTiming] {
        &self.phases
    }

    pub fn record_result<T, E>(
        &mut self,
        stage: StageMeta,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        if !self.enabled {
            return work();
        }
        let time_start = Instant::now();
        let result = work();
        let microseconds = time_start.elapsed().as_micros();

        self.add_completed(stage, microseconds);

        result
    }

    pub fn add_completed(&mut self, stage: StageMeta, microseconds: u128) {
        if !self.enabled {
            return;
        }
        self.add_phase(stage.label(), microseconds);
    }

    pub fn extend(&mut self, shared: &Self) {
        if self.enabled {
            for timing in &shared.phases {
                self.add_phase(timing.phase.clone(), timing.microseconds);
            }
        }
    }

    fn add_phase(&mut self, phase: String, microseconds: u128) {
        if let Some(existing) = self.phases.iter_mut().find(|timing| timing.phase == phase) {
            existing.microseconds = existing.microseconds.saturating_add(microseconds);
        } else {
            self.phases.push(PhaseTiming {
                phase,
                microseconds,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileTimings, SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};

    #[test]
    fn repeated_phase_measurements_aggregate_without_reordering() {
        let mut timings = CompileTimings::enabled();
        timings.add_completed(TOKENS_TO_SYNTAX_TREES, 7);
        timings.add_completed(SOURCE_FILES_TO_TOKENS, 13);
        timings.add_completed(TOKENS_TO_SYNTAX_TREES, 17);

        assert_eq!(timings.phases().len(), 2);
        assert_eq!(timings.phases()[0].phase, TOKENS_TO_SYNTAX_TREES.label());
        assert_eq!(timings.phases()[0].microseconds, 24);
        assert_eq!(timings.phases()[1].phase, SOURCE_FILES_TO_TOKENS.label());
    }

    #[test]
    fn generic_error_measurement_preserves_the_exact_error() {
        let mut timings = CompileTimings::enabled();
        let result: Result<(), &'static str> =
            timings.record_result(TOKENS_TO_SYNTAX_TREES, || Err("retained base"));

        assert_eq!(result, Err("retained base"));
        assert_eq!(timings.phases().len(), 1);
        assert_eq!(timings.phases()[0].phase, TOKENS_TO_SYNTAX_TREES.label());
    }

    #[test]
    fn default_collection_runs_work_without_retaining_measurements() {
        let mut timings = CompileTimings::default();
        assert_eq!(
            timings.record_result(TOKENS_TO_SYNTAX_TREES, || Ok::<_, ()>(7)),
            Ok(7)
        );
        timings.add_completed(SOURCE_FILES_TO_TOKENS, 3);
        assert!(timings.phases().is_empty());
    }

    #[test]
    fn shared_work_adds_to_local_measurements_without_enabling_collection() {
        let mut shared = CompileTimings::enabled();
        shared.add_completed(SOURCE_FILES_TO_TOKENS, 7);
        let mut local = CompileTimings::enabled();
        local.add_completed(SOURCE_FILES_TO_TOKENS, 3);
        local.extend(&shared);
        assert_eq!(local.phases()[0].microseconds, 10);
        assert_eq!(shared.phases()[0].microseconds, 7);
        let mut disabled = CompileTimings::default();
        disabled.extend(&shared);
        assert!(disabled.phases().is_empty());
    }
}
