use super::super::super::OptimizedVerifiedPhysicalPipelineError;
use crate::{
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedLiveRanges,
    StagedOptimizedVerifiedPhysicalPipeline, stage_active_resident_register_allocation,
    stage_allocation_recovery_function_relative_realization,
    stage_optimized_post_allocation_machine_plan,
};

pub(super) fn stage_active_resident(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let allocation = stage_active_resident_register_allocation(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    let machine = stage_optimized_post_allocation_machine_plan(&allocation)
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let realization = stage_allocation_recovery_function_relative_realization(
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(Box::new(
            allocation,
        )),
        machine,
    )
    .map_err(|error| {
        OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryFunctionRelative(Box::new(error))
    })?;
    Ok(
        StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery {
            realization: Box::new(realization),
        },
    )
}
