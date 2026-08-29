//! Independent live-range validation entrance.
//!
//! This join first replays liveness custody, then descends into exact
//! live-range reconstruction. Receipt construction and focused corruption
//! tests live below `validate/`; producer computation remains a sibling of
//! this entire validation subtree.

mod receipt;
mod replay;

use crate::{
    LiveRangeError, LiveRangePlan, ValidatedLiveRanges, ValidatedLiveness, validate_liveness,
};

pub fn validate_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
    plan: LiveRangePlan,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    revalidate_liveness_custody(selected, liveness)?;
    replay::replay_live_ranges(selected, liveness, plan)
}

pub(crate) fn revalidate_liveness_custody(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<(), LiveRangeError> {
    let replayed = validate_liveness(selected, liveness.plan().clone())
        .map_err(LiveRangeError::LivenessRevalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(LiveRangeError::LivenessReceiptMismatch);
    }
    Ok(())
}
