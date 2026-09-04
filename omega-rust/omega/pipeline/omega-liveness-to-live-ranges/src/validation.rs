use omega_regalloc::{ValidatedLiveRanges, validate_live_ranges};

use omega_selected_instructions_to_liveness::{
    StagedOptimizedLiveness, validate_optimized_liveness_custody,
};

use super::custody::live_range_custody_receipt;
use super::model::{OptimizedLiveRangeCustodyError, StagedOptimizedLiveRangeCustodyReceipt};

pub fn validate_optimized_live_range_custody(
    liveness: &StagedOptimizedLiveness,
    ranges: &ValidatedLiveRanges,
) -> Result<StagedOptimizedLiveRangeCustodyReceipt, OptimizedLiveRangeCustodyError> {
    let upstream =
        validate_optimized_liveness_custody(liveness.selected_stage(), liveness.liveness())
            .map_err(OptimizedLiveRangeCustodyError::UpstreamLiveness)?;
    let replayed = validate_live_ranges(
        liveness.selected_stage().selected(),
        liveness.liveness(),
        ranges.plan().clone(),
    )
    .map_err(OptimizedLiveRangeCustodyError::Revalidation)?;
    if replayed.receipt() != ranges.receipt() {
        return Err(OptimizedLiveRangeCustodyError::ReceiptMismatch);
    }
    Ok(live_range_custody_receipt(upstream, replayed.receipt()))
}
