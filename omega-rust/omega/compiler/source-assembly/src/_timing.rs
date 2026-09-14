use crate::pipeline::stage::StageMeta;
use artifacts::PhaseTiming;
use artifacts::allocations::AllocationDelta;
use artifacts::allocations::snapshot as allocation_snapshot;
use diagnostics::Diagnostic;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct CompileTimings {
    phases: Vec<PhaseTiming>,
}

impl CompileTimings {
    pub(super) fn phases(&self) -> &[PhaseTiming] {
        &self.phases
    }

    pub(super) fn record<T>(
        &mut self,
        stage: StageMeta,
        work: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        self.record_result(stage, work)
    }

    pub(super) fn record_result<T, E>(
        &mut self,
        stage: StageMeta,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let allocation_start = allocation_snapshot();
        let time_start = Instant::now();
        let result = work();
        let microseconds = time_start.elapsed().as_micros();
        let allocations = allocation_snapshot().delta_since(allocation_start);

        self.add_completed(stage, microseconds, allocations);

        result
    }

    pub(super) fn add_completed(
        &mut self,
        stage: StageMeta,
        microseconds: u128,
        allocations: AllocationDelta,
    ) {
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
    use super::*;
    use crate::pipeline::stage::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};

    #[test]
    fn repeated_phase_measurements_aggregate_without_reordering() {
        let mut timings = CompileTimings::default();
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
        let mut timings = CompileTimings::default();
        let result: Result<(), &'static str> =
            timings.record_result(TOKENS_TO_SYNTAX_TREES, || Err("retained base"));

        assert_eq!(result, Err("retained base"));
        assert_eq!(timings.phases().len(), 1);
        assert_eq!(timings.phases()[0].phase, TOKENS_TO_SYNTAX_TREES.label());
    }
}
