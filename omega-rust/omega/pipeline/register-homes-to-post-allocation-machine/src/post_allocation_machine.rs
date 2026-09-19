//! Optimizer module role: executable entrance. Current register homes to machine facts.
//!
//! Allocation replay supplies the same postcondition after every rewrite.
//! Machine analysis consumes only current facts and retains allocation evidence separately.
//! `model` owns the sealed plan and its custody receipt; `validation` replays
//! the custody join against the same allocation evidence.

mod model;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

pub use model::*;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::*;

use crate::{ValidatedPostAllocationMachinePlan, analyze_post_allocation_machine_plan};
use selected_instructions_to_register_homes::{
    AllocationEvidence, AllocationSource, ValidatedPreAllocationMachineEffects,
    analyze_machine_effects,
};

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

fn seal_staged_post_allocation_machine(
    source: AllocationEvidence,
    effects: ValidatedPreAllocationMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(source, &effects, &machine);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        custody,
    }
}

fn post_allocation_machine_custody(
    source: AllocationEvidence,
    effects: &ValidatedPreAllocationMachineEffects,
    machine: &ValidatedPostAllocationMachinePlan,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        machine: machine.receipt().identity(),
        function_count: machine.plan().functions.len(),
        instruction_count: machine.receipt().instruction_count(),
        operand_count: machine.receipt().operand_count(),
        unit_action_count: machine.receipt().unit_action_count(),
    }
}
