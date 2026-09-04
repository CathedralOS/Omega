use crate::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedOptimizedRegisterHomesAfterFixedViewCopies, StagedOptimizedSelectedInstructions,
    validate_optimized_register_home_after_fixed_view_copy_custody,
};

use super::StagedAllocationRecoverySourceCustodyReceipt;

pub(in crate::stages::realization::allocation_recovery_function_relative_realization) fn fixed_view_selected_stage(
    homes: &StagedOptimizedRegisterHomesAfterFixedViewCopies,
) -> &StagedOptimizedSelectedInstructions {
    homes
        .reanalysis_stage()
        .transformation_stage()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
}

pub(in crate::stages::realization::allocation_recovery_function_relative_realization) fn validate_fixed_view_source(
    homes: &StagedOptimizedRegisterHomesAfterFixedViewCopies,
) -> Result<
    StagedAllocationRecoverySourceCustodyReceipt,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    validate_optimized_register_home_after_fixed_view_copy_custody(
        homes.reanalysis_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map(StagedAllocationRecoverySourceCustodyReceipt::FixedViewCopies)
    .map_err(AllocationRecoveryFunctionRelativeRealizationError::FixedViewSource)
}
