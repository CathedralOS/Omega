use crate::pipeline::stage::StageMeta;
use omega_artifacts::PhaseTiming;
use omega_core::allocations::AllocationDelta;
use omega_core::allocations::snapshot as allocation_snapshot;
use psi_diagnostics::Diagnostic;
use std::time::Instant;

#[derive(Default)]
pub(super) struct CompileTimings {
    phases: Vec<PhaseTiming>,
}

impl CompileTimings {
    pub(super) fn record<T>(
        &mut self,
        stage: StageMeta,
        work: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
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
