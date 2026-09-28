//! Coordinate the physical stages for one validated target program.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and runs selection, analysis, allocation, and machine construction
//! once before realization. The backend owns the returned fragment-emission
//! input; [`error`] defines the coordinator's failure surface. Every admitted
//! selection uses the same canonical frame realization.

mod error;

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
pub use error::OptimizedVerifiedPhysicalPipelineError;
use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use optimization_core::{OptimizationExecutionPhase, PostTerminalOptimizationSelections};
use register_environment::baseline_target_register_environment;
use selected_instructions_to_register_homes::stage_register_allocation;
use selected_instructions_to_selected_instructions::optimize_selected_instructions;

/// Feed the physical stages into the backend's fragment-emission input,
/// retaining their checked program and replay evidence without a compiler wrapper.
pub fn stage_optimized_verified_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
    post_terminal: &PostTerminalOptimizationSelections,
) -> Result<StagedOptimizedFunctionFragmentEmissionSource, OptimizedVerifiedPhysicalPipelineError> {
    let retained_projection = optimized_target
        .optimized()
        .selections()
        .project_post_terminal();
    if retained_projection.selections() != post_terminal {
        return Err(OptimizedVerifiedPhysicalPipelineError::PostTerminalSelectionMismatch);
    }
    // This sequence cannot consume these phases yet. Rule catalogs for the
    // stages it does execute are admitted by those stages, not preflighted here.
    for phase in [
        OptimizationExecutionPhase::AbstractOperations,
        OptimizationExecutionPhase::TargetOperations,
        OptimizationExecutionPhase::PostAllocationMachine,
    ] {
        if !post_terminal.selections().for_phase(phase).is_empty() {
            return Err(OptimizedVerifiedPhysicalPipelineError::UnconsumedPostTerminalPhase(phase));
        }
    }
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
        .map(StagedOptimizedFunctionFragmentEmissionSource::from)
        .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)
}
