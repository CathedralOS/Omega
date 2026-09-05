use crate::{
    OptimizedPostAllocationMachineOptimizationError, StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use omega_register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use omega_selected_instructions_to_register_homes::{AllocationOutput, AllocationSource};

pub(super) fn replay_machine_source<'source>(
    source: &'source impl AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<AllocationOutput<'source>, OptimizedPostAllocationMachineOptimizationError> {
    let allocation = source.replay_allocation().map_err(|error| {
        OptimizedPostAllocationMachineOptimizationError::Source(
            OptimizedPostAllocationMachinePipelineError::Allocation(error),
        )
    })?;
    validate_optimized_post_allocation_machine_plan_custody(&allocation, machine)
        .map_err(OptimizedPostAllocationMachineOptimizationError::Source)?;
    Ok(allocation)
}
