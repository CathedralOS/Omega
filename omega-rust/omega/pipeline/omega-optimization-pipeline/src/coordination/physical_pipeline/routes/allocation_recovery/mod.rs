//! Optimizer module role: executable entrance. Plain realization after the allocation phase.

use super::super::OptimizedVerifiedPhysicalPipelineError;
use crate::{
    StagedOptimizedLiveRanges, StagedOptimizedVerifiedPhysicalPipeline,
    stage_allocation_recovery_function_relative_realization,
    stage_optimized_post_allocation_machine_plan, stage_register_allocation,
};

pub(in crate::coordination::physical_pipeline) fn stage_allocation_recovery_pipeline(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let allocation = stage_register_allocation(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    let machine = stage_optimized_post_allocation_machine_plan(&allocation.current())
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let realization = stage_allocation_recovery_function_relative_realization(allocation, machine)
        .map_err(|error| {
            OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryFunctionRelative(Box::new(
                error,
            ))
        })?;
    Ok(StagedOptimizedVerifiedPhysicalPipeline::from(realization))
}
