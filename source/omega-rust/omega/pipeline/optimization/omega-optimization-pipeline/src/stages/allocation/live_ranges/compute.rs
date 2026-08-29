use omega_regalloc::{ValidatedLiveRanges, analyze_live_ranges};

use crate::StagedOptimizedLiveness;

use super::model::OptimizedLiveRangeCustodyError;

pub(super) fn compute_live_ranges(
    liveness: &StagedOptimizedLiveness,
) -> Result<ValidatedLiveRanges, OptimizedLiveRangeCustodyError> {
    analyze_live_ranges(liveness.selected_stage().selected(), liveness.liveness())
        .map_err(OptimizedLiveRangeCustodyError::Analysis)
}
