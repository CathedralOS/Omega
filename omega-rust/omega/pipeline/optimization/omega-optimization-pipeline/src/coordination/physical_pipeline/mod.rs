//! Optimizer module role: executable entrance. Compiler-facing physical optimizer coordination.
//!
//! This entrance consumes validated target operations, reads the exact selected
//! phase set, and sends custody into one named route. [`input`] retains
//! compatibility helpers that compose provider-aware target lowering with this
//! entrance, [`model`] defines the returned carrier, [`error`] defines the
//! closed failure surface, and [`routes`] owns the lower route taxonomy.

mod error;
mod input;
mod model;
mod routes;

use crate::ValidatedOptimizedTargetOperations;
pub use error::OptimizedVerifiedPhysicalPipelineError;
pub(crate) use input::{
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_optimized_verified_physical_pipeline_with_provider_executions_and_installation,
};
pub use model::StagedOptimizedVerifiedPhysicalPipeline;

pub(crate) use routes::{
    ResolvedNonAllocationComposition, ResolvedPhysicalPhaseComposition,
    resolve_physical_phase_composition,
};
use routes::{stage_allocation_recovery_pipeline, stage_non_allocation_recovery_physical_pipeline};

pub fn stage_optimized_verified_physical_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let selections = optimized_target.optimized().selections();
    let composition =
        resolve_physical_phase_composition(selections, optimized_target.target().architecture)?;
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
