use crate::{
    FixedPrecoloredIntervalValidationReceipt, FixedPrecoloredSegmentHomeValidationReceipt,
    FixedPrecoloredSplitRequirementValidationReceipt,
};

use crate::AllocationLegalityCustodyReceipt;

use super::FixedPrecoloredSegmentHomeCustodyReceipt;

pub(super) const fn seal(
    upstream: AllocationLegalityCustodyReceipt,
    fixed: FixedPrecoloredIntervalValidationReceipt,
    requirements: FixedPrecoloredSplitRequirementValidationReceipt,
    homes: FixedPrecoloredSegmentHomeValidationReceipt,
) -> FixedPrecoloredSegmentHomeCustodyReceipt {
    FixedPrecoloredSegmentHomeCustodyReceipt {
        upstream,
        fixed,
        requirements,
        homes,
    }
}
