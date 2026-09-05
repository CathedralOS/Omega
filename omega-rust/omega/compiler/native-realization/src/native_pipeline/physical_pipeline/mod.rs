//! Optimizer module role: executable entrance. Compiler-facing physical optimizer coordination.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and runs selection, analysis, allocation, and machine construction
//! once before realization. [`model`] defines the returned carrier, [`error`]
//! defines the closed failure surface, and
//! [`routes`] owns the lower route taxonomy. The test-only [`input`] helper
//! composes target lowering to exercise the complete route in isolation.

mod error;
#[cfg(any(test, feature = "test-support"))]
mod input;
mod model;
mod phase_selections;
mod routes;
#[cfg(any(test, feature = "test-support"))]
mod test_support;

use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;
pub use error::OptimizedVerifiedPhysicalPipelineError;
#[cfg(any(test, feature = "test-support"))]
pub use input::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use model::StagedOptimizedVerifiedPhysicalPipeline;
use optimization_core::PostTerminalOptimizationSelections;
pub(crate) use phase_selections::PhysicalOptimizationPhaseSelections;
use register_environment::baseline_target_register_environment;
use selected_instructions_to_register_homes::{
    stage_optimized_live_ranges, stage_optimized_liveness, stage_register_allocation,
};

pub(crate) use routes::{
    ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan, resolve_physical_phase_composition,
};
use routes::{realize_allocated_program, realize_recovered_allocation};

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
    let phase_selections = PhysicalOptimizationPhaseSelections::project(post_terminal)?;
    let composition = resolve_physical_phase_composition(
        &phase_selections,
        optimized_target.target().architecture,
    )?;
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            optimized_target,
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    let allocation = stage_register_allocation(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &allocation.current(),
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    match composition {
        ResolvedPhysicalPhaseComposition::AllocationRecovery {
            post_allocation: Some(entry),
            ..
        } => realize_allocated_program(
            allocation,
            machine,
            ResolvedRealizationPlan::PostAllocationMachine { entry },
        ),
        ResolvedPhysicalPhaseComposition::AllocationRecovery {
            post_allocation: None,
            ..
        } => realize_recovered_allocation(allocation, machine),
        ResolvedPhysicalPhaseComposition::Realization(composition) => {
            realize_allocated_program(allocation, machine, composition)
        }
    }
}
