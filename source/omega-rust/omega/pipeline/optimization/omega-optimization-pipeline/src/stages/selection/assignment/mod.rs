//! Bounded target-operation assignment stage.
//!
//! Source lowering, assignment construction, retained custody, and independent
//! replay descend into named leaves. This entrance alone joins construction to
//! replay and admits the staged assignment carrier.

mod construction;
mod model;
mod source;
mod validation;

pub use model::*;
pub(crate) use source::{
    stage_optimized_assignment_with_provider_executions,
    stage_optimized_assignment_with_provider_executions_and_installation,
};
pub use validation::validate_optimized_assignment_custody;

use crate::ValidatedOptimizedTargetOperations;

/// Assign current physical homes while retaining the exact optimized target
/// carrier and independently replaying every copied stage root and function
/// provenance row before granting custody.
pub(crate) fn stage_optimized_assignment(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let (register_environment, assigned) =
        construction::construct_optimized_assignment(&optimized_target)?;
    let custody =
        validate_optimized_assignment_custody(&optimized_target, &register_environment, &assigned)
            .map_err(OptimizedAssignmentPipelineError::Custody)?;
    Ok(StagedOptimizedAssignedOperations {
        optimized_target,
        register_environment,
        assigned,
        custody,
    })
}
