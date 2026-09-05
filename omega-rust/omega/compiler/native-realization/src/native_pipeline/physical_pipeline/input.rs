//! Provider-aware admission into physical optimizer routing.

use abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use target::NativeTarget;

use abstract_operations_to_target_operations::lower_optimized_to_target_operations_with_provider_executions;

use super::{
    OptimizedVerifiedPhysicalPipelineError, StagedOptimizedVerifiedPhysicalPipeline,
    stage_optimized_verified_physical_pipeline,
};

/// Lower one verified optimized plan through every currently admitted
/// selected/physical validation stage. Phase routing is derived from the exact
/// retained build suite; callers cannot request or skip selected-lowering work
/// independently.
pub fn stage_optimized_verified_physical_pipeline_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let post_terminal = optimized.selections().project_post_terminal();
    let optimized_target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::TargetLowering)?;
    stage_optimized_verified_physical_pipeline(optimized_target, post_terminal.selections())
}
