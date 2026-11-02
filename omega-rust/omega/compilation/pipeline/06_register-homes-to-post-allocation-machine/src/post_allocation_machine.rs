//! Optimizer module role: executable entrance. Current register homes to machine facts.
//!
//! Allocation supplies the current program with its admitted facts, including
//! the program's machine effects, as representation data. The stage builds the baseline plan under the
//! legality rule, executes the post-allocation optimization phase over it
//! (`optimization`), then seals the result. This file owns the sealed plan,
//! the phase run it carries, and their custody receipt.

use crate::PostAllocationMachineError;
use crate::ValidatedPostAllocationMachinePlan;
use crate::optimization::{
    PostAllocationMachineOptimizationError, PostAllocationMachineOptimizationRun,
    execute_post_allocation_machine_optimizations,
};
use crate::plan::{
    PlanSources, compute_post_allocation_machine_plan, seal_post_allocation_machine_plan,
};
use optimization_core::{
    OptimizationSelectionIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use physical_instructions::PostAllocationMachineIdentity;
use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes::AllocatedProgramFacts;
use selected_instructions::PreAllocationMachineEffectPlan;
use std::sync::Arc;

pub fn stage_optimized_post_allocation_machine_plan(
    allocation: AllocatedProgramFacts<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedPostAllocationMachinePlan, OptimizedPostAllocationMachinePipelineError> {
    let mut machine = compute_post_allocation_machine_plan(&PlanSources {
        allocation,
        register_environment: environment.identity(),
        physical: environment.physical(),
        constraints: environment.constraints(),
    })
    .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    let optimization = execute_post_allocation_machine_optimizations(
        &mut machine,
        allocation.selected.plan,
        allocation.effects,
        selections,
        budget,
    )
    .map_err(OptimizedPostAllocationMachinePipelineError::Optimization)?;
    let machine = seal_post_allocation_machine_plan(machine)
        .map_err(OptimizedPostAllocationMachinePipelineError::PostAllocation)?;
    Ok(seal_staged_post_allocation_machine(
        Arc::clone(allocation.effects),
        machine,
        optimization,
    ))
}

fn seal_staged_post_allocation_machine(
    effects: Arc<PreAllocationMachineEffectPlan>,
    machine: ValidatedPostAllocationMachinePlan,
    optimization: PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachinePlan {
    let custody = post_allocation_machine_custody(&effects, &machine, &optimization);
    StagedOptimizedPostAllocationMachinePlan {
        effects,
        machine,
        optimization,
        custody,
    }
}

fn post_allocation_machine_custody(
    effects: &PreAllocationMachineEffectPlan,
    machine: &ValidatedPostAllocationMachinePlan,
    optimization: &PostAllocationMachineOptimizationRun,
) -> StagedOptimizedPostAllocationMachineCustodyReceipt {
    StagedOptimizedPostAllocationMachineCustodyReceipt {
        effects: effects.identity,
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

/// Home-aware machine facts over the current allocation, with the machine
/// effects they were built from and the post-allocation phase run that
/// produced them. This remains non-emission and non-publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachinePlan {
    effects: Arc<PreAllocationMachineEffectPlan>,
    machine: ValidatedPostAllocationMachinePlan,
    optimization: PostAllocationMachineOptimizationRun,
    custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
}

impl StagedOptimizedPostAllocationMachinePlan {
    borrowed_accessors! {
        effects: Arc<PreAllocationMachineEffectPlan>,
        machine: ValidatedPostAllocationMachinePlan,
        /// The post-allocation phase execution whose result `machine` is.
        optimization: PostAllocationMachineOptimizationRun,
        custody: StagedOptimizedPostAllocationMachineCustodyReceipt,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedOptimizedPostAllocationMachineCustodyReceipt {
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
    copied_accessors! {
        effects: selected_instructions::PreAllocationMachineEffectIdentity,
        machine: PostAllocationMachineIdentity,
        /// The post-allocation projection the phase executed.
        phase_selections: OptimizationSelectionIdentity,
        /// The baseline plan identity the phase started from.
        optimization_source: PostAllocationMachineIdentity,
        optimization_action_count: usize,
        function_count: usize,
        instruction_count: usize,
        operand_count: usize,
        unit_action_count: usize,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachinePipelineError {
    PostAllocation(PostAllocationMachineError),
    Optimization(PostAllocationMachineOptimizationError),
}

stage_error_display!(
    OptimizedPostAllocationMachinePipelineError,
    "optimized post-allocation machine staging failed"
);
