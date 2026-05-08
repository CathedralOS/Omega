use super::NativePlanPhaseTiming;
use omega_core::allocations::snapshot as allocation_snapshot;
use std::time::Instant;

pub(super) fn record_native_phase<T>(
    timings: &mut Vec<NativePlanPhaseTiming>,
    phase: &str,
    work: impl FnOnce() -> T,
) -> T {
    let allocation_start = allocation_snapshot();
    let time_start = Instant::now();
    let result = work();
    let microseconds = time_start.elapsed().as_micros();
    let allocations = allocation_snapshot().delta_since(allocation_start);

    timings.push(NativePlanPhaseTiming {
        phase: phase.to_owned(),
        microseconds,
        allocations,
    });

    result
}
