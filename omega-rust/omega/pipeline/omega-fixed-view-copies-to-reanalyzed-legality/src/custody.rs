use omega_regalloc::{ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness};

use omega_allocation_legality_to_fixed_view_copies::StagedOptimizedFixedViewCopyCustodyReceipt;

use super::model::StagedOptimizedSelectedReanalysisCustodyReceipt;

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
        function_count: legality.receipt().function_count(),
        virtual_register_count: legality.receipt().virtual_register_count(),
        entry_transition_count: legality.receipt().entry_transition_count(),
    }
}
