use crate::LiveRangeValidationReceipt;

use crate::StagedOptimizedLivenessCustodyReceipt;

use super::StagedOptimizedLiveRangeCustodyReceipt;

pub(super) fn live_range_custody_receipt(
    upstream: StagedOptimizedLivenessCustodyReceipt,
    ranges: LiveRangeValidationReceipt,
) -> StagedOptimizedLiveRangeCustodyReceipt {
    StagedOptimizedLiveRangeCustodyReceipt {
        core: upstream.core(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: ranges.identity(),
    }
}
