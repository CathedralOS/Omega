use omega_regalloc::{
    FixedPrecoloredIntervalValidationReceipt, FixedPrecoloredSegmentHomeValidationReceipt,
    FixedPrecoloredSplitRequirementValidationReceipt,
};

use omega_live_ranges_to_allocation_legality::StagedOptimizedAllocationLegalityCustodyReceipt;

use super::StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt;

pub(super) const fn seal(
    upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    fixed: FixedPrecoloredIntervalValidationReceipt,
    requirements: FixedPrecoloredSplitRequirementValidationReceipt,
    homes: FixedPrecoloredSegmentHomeValidationReceipt,
) -> StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt {
        upstream,
        fixed,
        requirements,
        homes,
    }
}
