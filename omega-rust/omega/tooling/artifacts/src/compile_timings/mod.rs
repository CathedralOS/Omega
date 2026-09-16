//! Compile timings: the phase ladder every stage records into, and the
//! accumulator that carries those records to the timing report.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingCategory {
    Pipeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageMeta {
    pub name: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub category: TimingCategory,
}

impl StageMeta {
    pub const fn new(
        name: &'static str,
        input: &'static str,
        output: &'static str,
        category: TimingCategory,
    ) -> Self {
        Self {
            name,
            input,
            output,
            category,
        }
    }

    pub fn label(self) -> String {
        format!("{}: {} -> {}", self.name, self.input, self.output)
    }
}

pub const SOURCE_FILES_TO_TOKENS: StageMeta = StageMeta::new(
    "Stage 01",
    "SourceFiles",
    "TokenStreams",
    TimingCategory::Pipeline,
);

pub const TOKENS_TO_SYNTAX_TREES: StageMeta = StageMeta::new(
    "Stage 02",
    "TokenStreams",
    "SyntaxTrees",
    TimingCategory::Pipeline,
);

pub const SYNTAX_TREES_TO_SYMBOL_RESOLVED_TREES: StageMeta = StageMeta::new(
    "Stage 03",
    "SyntaxTrees",
    "SymbolResolvedTrees",
    TimingCategory::Pipeline,
);

pub const SYMBOL_RESOLVED_TREES_TO_TYPED_TREES: StageMeta = StageMeta::new(
    "Stage 04",
    "SymbolResolvedTrees",
    "TypedTrees",
    TimingCategory::Pipeline,
);

pub const TYPED_TREES_TO_CHECKED_TREES: StageMeta = StageMeta::new(
    "Stage 05",
    "TypedTrees",
    "CheckedTrees",
    TimingCategory::Pipeline,
);

use crate::allocations::AllocationDelta;
use diagnostics::Diagnostic;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
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

    pub fn phases(&self) -> &[PhaseTiming] {
        &self.phases
    }

    pub fn record<T>(
        &mut self,
        stage: StageMeta,
        work: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        self.record_result(stage, work)
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

        self.add_completed(stage, microseconds, AllocationDelta::default());

        result
    }

    pub fn add_completed(
        &mut self,
        stage: StageMeta,
        microseconds: u128,
        allocations: AllocationDelta,
    ) {
        if !self.enabled {
            return;
        }
        let phase = stage.label();
        if let Some(existing) = self.phases.iter_mut().find(|timing| timing.phase == phase) {
            existing.microseconds = existing.microseconds.saturating_add(microseconds);
            existing.allocations.allocation_calls = existing
                .allocations
                .allocation_calls
                .saturating_add(allocations.allocation_calls);
            existing.allocations.deallocation_calls = existing
                .allocations
                .deallocation_calls
                .saturating_add(allocations.deallocation_calls);
            existing.allocations.allocated_bytes = existing
                .allocations
                .allocated_bytes
                .saturating_add(allocations.allocated_bytes);
            existing.allocations.deallocated_bytes = existing
                .allocations
                .deallocated_bytes
                .saturating_add(allocations.deallocated_bytes);
        } else {
            self.phases.push(PhaseTiming {
                phase,
                microseconds,
                allocations,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocationDelta, CompileTimings};
    use crate::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};

    #[test]
    fn repeated_phase_measurements_aggregate_without_reordering() {
        let mut timings = CompileTimings::enabled();
        timings.add_completed(
            TOKENS_TO_SYNTAX_TREES,
            7,
            AllocationDelta {
                allocation_calls: 1,
                allocated_bytes: 11,
                ..AllocationDelta::default()
            },
        );
        timings.add_completed(
            SOURCE_FILES_TO_TOKENS,
            13,
            AllocationDelta {
                deallocation_calls: 2,
                deallocated_bytes: 5,
                ..AllocationDelta::default()
            },
        );
        timings.add_completed(
            TOKENS_TO_SYNTAX_TREES,
            17,
            AllocationDelta {
                allocation_calls: 3,
                allocated_bytes: 19,
                ..AllocationDelta::default()
            },
        );

        assert_eq!(timings.phases().len(), 2);
        assert_eq!(timings.phases()[0].phase, TOKENS_TO_SYNTAX_TREES.label());
        assert_eq!(timings.phases()[0].microseconds, 24);
        assert_eq!(timings.phases()[0].allocations.allocation_calls, 4);
        assert_eq!(timings.phases()[0].allocations.allocated_bytes, 30);
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
        timings.add_completed(SOURCE_FILES_TO_TOKENS, 3, AllocationDelta::default());
        assert!(timings.phases().is_empty());
    }
}
