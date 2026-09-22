//! Optimizer module role: executable entrance. Current register homes to machine facts.
//!
//! Allocation replay supplies the same postcondition after every rewrite.
//! Machine analysis consumes only current facts and retains allocation evidence separately.
//! This file owns the sealed plan and its custody receipt; `validation` replays
//! the custody join against the same allocation evidence.

#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::*;
pub use validation::*;

use crate::PostAllocationMachineError;
use crate::{ValidatedPostAllocationMachinePlan, analyze_post_allocation_machine_plan};
use physical_instructions::PostAllocationMachineIdentity;
use selected_instructions_to_register_homes::AllocationReplayError;
use selected_instructions_to_register_homes::MachineEffectStageError;
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

/// Home-aware machine facts joined only through independently replayed source
/// custody. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    effects: ValidatedPreAllocationMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
    custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub const fn effects(&self) -> &ValidatedPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn machine(&self) -> &ValidatedPostAllocationMachinePlan {
        &self.machine
    }

    pub const fn custody(&self) -> &StagedOptimizedPostAllocationMachineCustodyReceipt {
        &self.custody
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachineCustodyReceipt {
    source: AllocationEvidence,
    effects: selected_instructions::PreAllocationMachineEffectIdentity,
    machine: PostAllocationMachineIdentity,
    function_count: usize,
    instruction_count: usize,
    operand_count: usize,
    unit_action_count: usize,
}

impl StagedOptimizedPostAllocationMachineCustodyReceipt {
    pub const fn source(&self) -> &AllocationEvidence {
        &self.source
    }
    pub const fn effects(&self) -> selected_instructions::PreAllocationMachineEffectIdentity {
        self.effects
    }
    pub const fn machine(&self) -> PostAllocationMachineIdentity {
        self.machine
    }
    pub const fn function_count(&self) -> usize {
        self.function_count
    }
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }
    pub const fn operand_count(&self) -> usize {
        self.operand_count
    }
    pub const fn unit_action_count(&self) -> usize {
        self.unit_action_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachinePipelineError {
    Allocation(AllocationReplayError),
    MachineEffects(MachineEffectStageError),
    PostAllocation(PostAllocationMachineError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostAllocationMachinePipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-allocation machine staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostAllocationMachinePipelineError {}
