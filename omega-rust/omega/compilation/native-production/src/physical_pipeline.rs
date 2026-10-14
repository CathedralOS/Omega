//! Coordinate the physical stages for one validated target program.
//!
//! This entrance consumes validated target operations and runs selection,
//! analysis, allocation, and machine construction
//! once before realization. The backend owns the returned fragment-emission
//! input; [`error`] defines the coordinator's failure surface. Every admitted
//! selection uses the same canonical frame realization. Selections travel with
//! the checked products; each stage owns execution or refusal of its phase.

mod error;

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
pub(crate) use error::OptimizedVerifiedPhysicalPipelineError;
use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use register_environment::baseline_target_register_environment;
use selected_instructions_to_register_homes::stage_register_allocation;
use selected_instructions_to_selected_instructions::optimize_selected_instructions;

/// Feed the physical stages into the backend's fragment-emission input,
/// retaining their checked program and replay evidence without a compiler wrapper.
pub(crate) fn stage_optimized_verified_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedFunctionFragmentEmissionSource, OptimizedVerifiedPhysicalPipelineError> {
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
