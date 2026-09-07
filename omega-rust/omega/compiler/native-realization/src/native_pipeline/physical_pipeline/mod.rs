//! Optimizer module role: executable entrance. Compiler-facing physical optimizer coordination.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and runs selection, analysis, allocation, and machine construction
//! once before realization. [`model`] defines the returned carrier, [`error`]
//! defines the closed failure surface. Every admitted selection uses the same
//! canonical frame realization. The test-only [`input`] helper
//! composes target lowering to exercise the complete route in isolation.

mod error;
#[cfg(any(test, feature = "test-support"))]
mod input;
mod model;
mod phase_selections;
#[cfg(any(test, feature = "test-support"))]
mod test_support;

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
pub use error::OptimizedVerifiedPhysicalPipelineError;
#[cfg(any(test, feature = "test-support"))]
pub use input::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use model::StagedOptimizedVerifiedPhysicalPipeline;
use optimization_core::PostTerminalOptimizationSelections;
use phase_selections::validate_physical_selections;
use register_environment::baseline_target_register_environment;
use selected_instructions_to_register_homes::stage_register_allocation;
use selected_instructions_to_selected_instructions::optimize_selected_instructions;

pub fn stage_optimized_verified_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
    post_terminal: &PostTerminalOptimizationSelections,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let retained_projection = optimized_target
        .optimized()
        .selections()
        .project_post_terminal();
    if retained_projection.selections() != post_terminal {
        return Err(OptimizedVerifiedPhysicalPipelineError::PostTerminalSelectionMismatch);
    }
    validate_physical_selections(post_terminal, optimized_target.target().architecture)?;
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            optimized_target,
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let selected = optimize_selected_instructions(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)?;
    let allocation = stage_register_allocation(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &allocation.current(),
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let budget = allocation.current().budget_per_pass();
    machine_emission::stage_fixed_frame_function_relative_realization(allocation, machine, budget)
        .map(StagedOptimizedVerifiedPhysicalPipeline::from)
        .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)
}
