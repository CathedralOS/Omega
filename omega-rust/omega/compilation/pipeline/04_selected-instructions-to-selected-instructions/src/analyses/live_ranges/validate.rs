//! Independent live-range validation entrance.
//!
//! The liveness input is a `ValidatedLiveness`: its own analysis replayed it
//! when it was produced, so this entrance checks only that it belongs to the
//! selected program before descending into exact live-range reconstruction.
//! Receipt construction and focused corruption tests live below `validate/`;
//! producer computation remains a sibling of this entire validation subtree.

mod receipt;
mod replay;

pub(crate) use receipt::admit;

use crate::{LiveRangeError, ValidatedLiveRanges, ValidatedLiveness};
use selected_instructions::LiveRangePlan;

pub fn validate_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
    plan: LiveRangePlan,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    replay::replay_live_ranges(selected, liveness, plan, &[], &[])
}

/// Reject a liveness plan computed for another selected program before the
/// producer reads it.
pub(super) fn require_liveness_of(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<(), LiveRangeError> {
    if liveness.plan().selected != selected.selected_identity()
        || liveness.receipt().selected() != selected.selected_identity()
    {
        return Err(LiveRangeError::RootMismatch);
    }
    Ok(())
}
