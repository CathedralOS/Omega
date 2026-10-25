use crate::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness};

use crate::StagedOptimizedFixedViewCopyCustodyReceipt;

use super::StagedOptimizedSelectedReanalysisCustodyReceipt;

pub(super) fn selected_reanalysis_custody_receipt(
    source: StagedOptimizedFixedViewCopyCustodyReceipt,
    liveness: &ValidatedLiveness,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
    StagedOptimizedSelectedReanalysisCustodyReceipt {
        source,
        transformed_liveness: liveness.receipt().identity(),
        transformed_ranges: ranges.receipt().identity(),
        transformed_legality: legality.receipt().identity(),
        allocator_availability: legality.receipt().allocator_availability(),
    }
}
