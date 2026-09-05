//! Optimizer module role: executable entrance. Plain realization after the allocation phase.

use super::super::OptimizedVerifiedPhysicalPipelineError;
use crate::StagedOptimizedVerifiedPhysicalPipeline;
use machine_emission::stage_allocation_recovery_function_relative_realization;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_instructions_to_register_homes::RetainedAllocation;

pub(in crate::native_pipeline::physical_pipeline) fn realize_recovered_allocation(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let realization = stage_allocation_recovery_function_relative_realization(allocation, machine)
        .map_err(|error| {
            OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryFunctionRelative(Box::new(
                error,
            ))
        })?;
    Ok(StagedOptimizedVerifiedPhysicalPipeline::from(realization))
}
