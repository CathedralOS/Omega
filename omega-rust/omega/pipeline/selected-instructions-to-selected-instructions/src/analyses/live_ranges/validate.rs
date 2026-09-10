//! Independent live-range validation entrance.
//!
//! Every public call first replays liveness custody, then descends into exact
//! live-range reconstruction. The producer may retain this prerequisite only
//! across its computation on the same borrowed inputs. It cannot substitute
//! another selected program or liveness plan at the final validation join.
//! Receipt construction and focused corruption
//! tests live below `validate/`; producer computation remains a sibling of
//! this entire validation subtree.

mod receipt;
mod replay;

use crate::{
    LiveRangeError, LiveRangePlan, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedSelectedAnalysis, validate_liveness,
};

pub fn validate_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
    plan: LiveRangePlan,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    RevalidatedLiveness::new(selected, liveness)?.validate(plan)
}

/// Invocation-local custody, not a reusable receipt or retained analysis cache.
/// The sealed selected input and liveness expose no mutation through shared
/// borrows. Private fields prevent re-binding an admitted pair before replay.
pub(super) struct RevalidatedLiveness<'input, Selected> {
    selected: &'input Selected,
    liveness: &'input ValidatedLiveness,
}

impl<'input, Selected: ValidatedSelectedAnalysis> RevalidatedLiveness<'input, Selected> {
    pub(super) fn new(
        selected: &'input Selected,
        liveness: &'input ValidatedLiveness,
    ) -> Result<Self, LiveRangeError> {
        revalidate_liveness_custody(selected, liveness)?;
        Ok(Self { selected, liveness })
    }

    pub(super) fn validate(
        self,
        plan: LiveRangePlan,
    ) -> Result<ValidatedLiveRanges, LiveRangeError> {
        replay::replay_live_ranges(self.selected, self.liveness, plan)
    }
}

fn revalidate_liveness_custody(
    selected: &impl ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<(), LiveRangeError> {
    #[cfg(test)]
    super::LIVENESS_REPLAYS.set(super::LIVENESS_REPLAYS.get() + 1);
    let replayed = validate_liveness(selected, liveness.plan().clone())
        .map_err(LiveRangeError::LivenessRevalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(LiveRangeError::LivenessReceiptMismatch);
    }
    Ok(())
}
