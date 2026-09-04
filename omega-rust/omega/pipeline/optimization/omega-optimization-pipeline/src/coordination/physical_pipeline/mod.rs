//! Optimizer module role: executable entrance. Compiler-facing physical optimizer coordination.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and sends custody into one named route. [`model`] defines the
//! returned carrier, [`error`] defines the closed failure surface, and
//! [`routes`] owns the lower route taxonomy. The test-only [`input`] helper
//! composes target lowering to exercise the complete route in isolation.

mod error;
#[cfg(test)]
mod input;
mod model;
mod routes;

use crate::ValidatedOptimizedTargetOperations;
pub use error::OptimizedVerifiedPhysicalPipelineError;
#[cfg(test)]
pub(crate) use input::stage_optimized_verified_physical_pipeline_with_provider_executions;
pub use model::StagedOptimizedVerifiedPhysicalPipeline;
use omega_optimization_core::PostTerminalOptimizationSelections;

pub(crate) use routes::{
    ResolvedNonAllocationComposition, ResolvedPhysicalPhaseComposition,
    resolve_physical_phase_composition,
};
use routes::{stage_allocation_recovery_pipeline, stage_non_allocation_recovery_physical_pipeline};

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
    let composition =
        resolve_physical_phase_composition(post_terminal, optimized_target.target().architecture)?;
    match composition {
        ResolvedPhysicalPhaseComposition::AllocationRecovery {
            rule,
            post_allocation,
        } => stage_allocation_recovery_pipeline(optimized_target, rule, post_allocation),
        ResolvedPhysicalPhaseComposition::NonAllocation(composition) => {
            stage_non_allocation_recovery_physical_pipeline(optimized_target, composition)
        }
    }
}
