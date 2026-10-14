//! Optimizer module role: executable entrance. Current register homes to machine facts.
//!
//! Allocation supplies the same current-program view after every rewrite.
//! Machine analysis consumes only current facts and retains allocation
//! evidence separately. The stage builds the baseline plan under the
//! legality rule, executes the post-allocation optimization phase over it
//! (`optimization`), then seals the result. This file owns the sealed plan,
//! the phase run it carries, and their custody receipt.

use crate::PostAllocationMachineError;
use crate::ValidatedPostAllocationMachinePlan;
use crate::optimization::{
    PostAllocationMachineOptimizationError, PostAllocationMachineOptimizationRun,
    execute_post_allocation_machine_optimizations,
};
use crate::plan::{compute_post_allocation_machine_plan, seal_post_allocation_machine_plan};
use optimization_core::OptimizationSelectionIdentity;
use physical_instructions::PostAllocationMachineIdentity;
use selected_instructions_to_register_homes::MachineEffectStageError;
use selected_instructions_to_register_homes::{
    AllocationEvidence, AllocationOutput, ValidatedPreAllocationMachineEffects,
    analyze_machine_effects,
};

pub fn stage_optimized_post_allocation_machine_plan(
    allocation: &AllocationOutput<'_>,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let selected = allocation.selected();
    let environment = allocation.register_environment();
    let effects = analyze_machine_effects(selected, environment)
        .map_err(OptimizedPostAllocationMachinePipelineError::MachineEffects)?;
    let mut machine = compute_post_allocation_machine_plan(
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
    let optimization = execute_post_allocation_machine_optimizations(
        &mut machine,
        allocation.selected_plan(),
        &effects,
        allocation.selections(),
        allocation.budget_per_pass(),
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::Optimization)?;
    let machine = seal_post_allocation_machine_plan(machine)
        .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        allocation.evidence().clone(),
        effects,
        machine,
        optimization,
    ))
}

fn seal_staged_post_allocation_machine(
    source: AllocationEvidence,
    effects: ValidatedPreAllocationMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
    optimization: PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(source, &effects, &machine, &optimization);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        optimization,
        custody,
    }
}

fn post_allocation_machine_custody(
    source: AllocationEvidence,
    effects: &ValidatedPreAllocationMachineEffects,
    machine: &ValidatedPostAllocationMachinePlan,
    optimization: &PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        source,
        effects: effects.receipt().identity(),
        machine: machine.receipt().identity(),
        phase_selections: optimization.phase_selections(),
        optimization_source: optimization.source(),
        optimization_action_count: optimization.action_count(),
        function_count: machine.plan().functions.len(),
        instruction_count: machine.receipt().instruction_count(),
        operand_count: machine.receipt().operand_count(),
        unit_action_count: machine.receipt().unit_action_count(),
    }
}

/// Home-aware machine facts over the current allocation, with the allocation
/// evidence they were built from and the post-allocation phase run that
/// produced them. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    effects: ValidatedPreAllocationMachineEffects,
    machine: ValidatedPostAllocationMachinePlan,
    optimization: PostAllocationMachineOptimizationRun,
    custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    pub const fn effects(&self) -> &ValidatedPreAllocationMachineEffects {
        &self.effects
    }

    pub const fn machine(&self) -> &ValidatedPostAllocationMachinePlan {
        &self.machine
    }

    /// The post-allocation phase execution whose result `machine` is.
    pub const fn optimization(&self) -> &PostAllocationMachineOptimizationRun {
        &self.optimization
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
    phase_selections: OptimizationSelectionIdentity,
    optimization_source: PostAllocationMachineIdentity,
    optimization_action_count: usize,
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
    /// The post-allocation projection the phase executed.
    pub const fn phase_selections(&self) -> OptimizationSelectionIdentity {
        self.phase_selections
    }
    /// The baseline plan identity the phase started from.
    pub const fn optimization_source(&self) -> PostAllocationMachineIdentity {
        self.optimization_source
    }
    pub const fn optimization_action_count(&self) -> usize {
        self.optimization_action_count
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
    MachineEffects(MachineEffectStageError),
    PostAllocation(PostAllocationMachineError),
    Optimization(PostAllocationMachineOptimizationError),
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
