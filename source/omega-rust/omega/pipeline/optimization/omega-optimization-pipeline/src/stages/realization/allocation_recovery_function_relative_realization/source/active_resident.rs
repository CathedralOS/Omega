use crate::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedSelectedInstructions,
    validate_optimized_active_resident_rematerialization,
};

use super::StagedAllocationRecoverySourceCustodyReceipt;

pub(in crate::stages::realization::allocation_recovery_function_relative_realization) fn active_resident_selected_stage(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> &StagedOptimizedSelectedInstructions {
    source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
}

pub(in crate::stages::realization::allocation_recovery_function_relative_realization) fn validate_active_resident_source(
    source: &StagedOptimizedActiveResidentRematerialization,
) -> Result<
    StagedAllocationRecoverySourceCustodyReceipt,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    validate_optimized_active_resident_rematerialization(source)
        .map(StagedAllocationRecoverySourceCustodyReceipt::ActiveResidentRematerialization)
        .map_err(AllocationRecoveryFunctionRelativeRealizationError::ActiveResidentSource)
}
