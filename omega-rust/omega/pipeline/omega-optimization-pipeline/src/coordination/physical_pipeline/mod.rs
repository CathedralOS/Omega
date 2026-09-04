//! Optimizer module role: executable entrance. Compiler-facing physical optimizer coordination.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and runs common selection and analysis before allocation dispatch. [`model`] defines the
//! returned carrier, [`error`] defines the closed failure surface, and
//! [`routes`] owns the lower route taxonomy. The test-only [`input`] helper
//! composes target lowering to exercise the complete route in isolation.

mod error;
#[cfg(test)]
mod input;
mod model;
mod phase_selections;
mod routes;

use crate::{
    ValidatedOptimizedTargetOperations, baseline_target_register_environment,
    stage_optimized_live_ranges, stage_optimized_liveness,
};
pub use error::OptimizedVerifiedPhysicalPipelineError;
#[cfg(test)]
pub(crate) use input::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use model::StagedOptimizedVerifiedPhysicalPipeline;
use omega_optimization_core::PostTerminalOptimizationSelections;
pub(crate) use phase_selections::PhysicalOptimizationPhaseSelections;

pub(crate) use routes::{
    ResolvedPhysicalPhaseComposition, ResolvedRealizationPlan, resolve_physical_phase_composition,
};
use routes::{stage_allocation_and_realization, stage_allocation_recovery_pipeline};

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
        omega_target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            optimized_target,
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    match composition {
        ResolvedPhysicalPhaseComposition::AllocationRecovery {
            post_allocation: Some(entry),
            ..
        } => stage_allocation_and_realization(
            ranges,
            ResolvedRealizationPlan::PostAllocationMachine { entry },
        ),
        ResolvedPhysicalPhaseComposition::AllocationRecovery {
            rule,
            post_allocation: None,
        } => stage_allocation_recovery_pipeline(ranges, rule),
        ResolvedPhysicalPhaseComposition::Realization(composition) => {
            stage_allocation_and_realization(ranges, composition)
        }
    }
}
