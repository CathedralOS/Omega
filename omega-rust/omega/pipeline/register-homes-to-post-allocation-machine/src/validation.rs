use super::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    post_allocation_machine_custody,
};
use crate::validate_post_allocation_machine_plan;
use selected_instructions_to_register_homes::AllocationSource;
use selected_instructions_to_register_homes::validate_machine_effects;

pub fn validate_optimized_post_allocation_machine_plan_custody(
    source: &impl AllocationSource,
    staged: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    OptimizedPostAllocationMachinePipelineError,
> {
    let allocation = source
        .replay_allocation()
        .map_err(OptimizedPostAllocationMachinePipelineError::Allocation)?;
    let selected = allocation.selected();
    let environment = allocation.register_environment();
    validate_machine_effects(selected, environment, staged.effects())
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let replayed = validate_post_allocation_machine_plan(
        selected,
        staged.effects(),
        allocation.ranges(),
        allocation.legality(),
        allocation.homes(),
        allocation.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        staged.machine().plan().clone(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    if &replayed != staged.machine() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    let receipt =
        post_allocation_machine_custody(allocation.evidence().clone(), staged.effects(), &replayed);
    if &receipt != staged.custody() {
        return Err(OptimizedPostAllocationMachinePipelineError::ReceiptMismatch);
    }
    Ok(receipt)
}
