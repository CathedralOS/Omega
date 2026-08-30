//! Optimizer module role: executable entrance. CFG-aware live-range staging.
//!
//! This entrance owns the analysis-to-independent-replay join over complete
//! liveness custody. No interval or interference fact escapes before replay.

mod compute;
mod custody;
mod model;
mod validation;

pub use model::*;
pub use validation::validate_optimized_live_range_custody;

use crate::StagedOptimizedLiveness;

pub fn stage_optimized_live_ranges(
    liveness: StagedOptimizedLiveness,
) -> Result<StagedOptimizedLiveRanges, OptimizedLiveRangeCustodyError> {
    let ranges = compute::compute_live_ranges(&liveness)?;
    let custody = validate_optimized_live_range_custody(&liveness, &ranges)?;
    Ok(StagedOptimizedLiveRanges {
        liveness,
        ranges,
        custody,
    })
}
