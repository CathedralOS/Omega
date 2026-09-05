//! Optimizer module role: executable entrance. Current register homes to machine facts.
//!
//! Allocation replay supplies the same postcondition after every rewrite.
//! Machine analysis consumes only current facts and retains allocation evidence separately.

use super::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    seal_staged_post_allocation_machine,
};
use crate::analyze_post_allocation_machine_plan;
use omega_selected_instructions_to_machine_effects::analyze_machine_effects;
use omega_selected_instructions_to_register_homes::AllocationSource;

pub fn stage_optimized_post_allocation_machine_plan(
    source: &impl AllocationSource,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let allocation = source
        .replay_allocation()
        .map_err(OptimizedPostAllocationMachinePipelineError::Allocation)?;
    let selected = allocation.selected();
    let environment = allocation.register_environment();
    let effects = analyze_machine_effects(selected, environment)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let machine = analyze_post_allocation_machine_plan(
        selected,
        &effects,
        allocation.ranges(),
        allocation.legality(),
        allocation.homes(),
        allocation.post_allocation_manifest(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        allocation.evidence().clone(),
        effects,
        machine,
    ))
}
