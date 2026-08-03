use omega_backend_plan::BackendPlanPhaseTiming;
use omega_core::allocations::snapshot as allocation_snapshot;
use psi_arena::Arena;
use std::time::Instant;

pub(super) fn record_backend_phase<T>(
    timings: &mut Arena<BackendPlanPhaseTiming>,
    phase: &'static str,
    work: impl FnOnce() -> T,
) -> T {
    let allocation_start = allocation_snapshot();
    let time_start = Instant::now();
    let result = work();
    let microseconds = time_start.elapsed().as_micros();
    let allocations = allocation_snapshot().delta_since(allocation_start);

    timings.append(BackendPlanPhaseTiming {
        phase,
        microseconds,
        allocations,
    });

    result
}
