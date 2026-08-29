use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;

use crate::{
    lower_optimized_to_target_operations_with_provider_executions,
    lower_optimized_to_target_operations_with_provider_executions_and_installation,
};

use super::{
    OptimizedAssignmentPipelineError, StagedOptimizedAssignedOperations, stage_optimized_assignment,
};

/// Lower and assign one optimized plan without exposing a bare target plan to
/// compiler coordination.
pub(crate) fn stage_optimized_assignment_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let optimized_target = lower_optimized_to_target_operations_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map_err(OptimizedAssignmentPipelineError::TargetLowering)?;
    stage_optimized_assignment(optimized_target)
}

/// Lower and assign one optimized plan with one exact checked-provider
/// installation. This is the installation-bearing form of the same canonical
/// assignment stage, not a second native route.
pub(crate) fn stage_optimized_assignment_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: AdmittedProviderInstallation,
) -> Result<StagedOptimizedAssignedOperations, OptimizedAssignmentPipelineError> {
    let optimized_target =
        lower_optimized_to_target_operations_with_provider_executions_and_installation(
            optimized,
            target,
            settlements,
            installation,
        )
        .map_err(OptimizedAssignmentPipelineError::TargetLowering)?;
    stage_optimized_assignment(optimized_target)
}
