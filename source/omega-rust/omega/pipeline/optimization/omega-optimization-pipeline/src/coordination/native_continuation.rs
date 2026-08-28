use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;

use crate::{
    OptimizedAssignmentPipelineError, OptimizedVerifiedPhysicalPipelineError,
    StagedOptimizedAssignedOperations, StagedOptimizedVerifiedPhysicalPipeline,
};

/// The single optimizer-owned continuation from verified, target-neutral
/// optimization into native realization.
///
/// `CoverageFallbackAssigned` is a temporary publication adapter for operation
/// shapes not yet represented by the selected-instruction pipeline. It is not
/// a second architecture or an optimization mode. `SelectedPhysical` is the
/// mandatory destination and remains fail-closed before publication. The
/// compiler enters through this type rather than selecting either incomplete
/// implementation internally.
#[derive(Debug)]
pub enum StagedOptimizedNativeContinuation {
    CoverageFallbackAssigned(StagedOptimizedAssignedOperations),
    SelectedPhysical(StagedOptimizedVerifiedPhysicalPipeline),
}

#[derive(Debug)]
pub enum OptimizedNativeContinuationError {
    CoverageFallbackAssigned(OptimizedAssignmentPipelineError),
    SelectedPhysical(OptimizedVerifiedPhysicalPipelineError),
}

impl std::fmt::Display for OptimizedNativeContinuationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized native continuation failed: {self:?}")
    }
}

impl std::error::Error for OptimizedNativeContinuationError {}

pub fn stage_optimized_native_continuation_with_provider_executions(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<StagedOptimizedNativeContinuation, OptimizedNativeContinuationError> {
    if optimized.selections().is_empty() {
        return crate::assignment::stage_optimized_assignment_with_provider_executions(
            optimized,
            target,
            settlements,
        )
        .map(StagedOptimizedNativeContinuation::CoverageFallbackAssigned)
        .map_err(OptimizedNativeContinuationError::CoverageFallbackAssigned);
    }
    crate::physical_pipeline::stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        target,
        settlements,
    )
    .map(StagedOptimizedNativeContinuation::SelectedPhysical)
    .map_err(OptimizedNativeContinuationError::SelectedPhysical)
}

pub fn stage_optimized_native_continuation_with_provider_executions_and_installation(
    optimized: ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: AdmittedProviderInstallation,
) -> Result<StagedOptimizedNativeContinuation, OptimizedNativeContinuationError> {
    if optimized.selections().is_empty() {
        return crate::assignment::stage_optimized_assignment_with_provider_executions_and_installation(
            optimized,
            target,
            settlements,
            installation,
        )
        .map(StagedOptimizedNativeContinuation::CoverageFallbackAssigned)
        .map_err(OptimizedNativeContinuationError::CoverageFallbackAssigned);
    }
    crate::physical_pipeline::stage_optimized_verified_physical_pipeline_with_provider_executions_and_installation(
        optimized,
        target,
        settlements,
        installation,
    )
    .map(StagedOptimizedNativeContinuation::SelectedPhysical)
    .map_err(OptimizedNativeContinuationError::SelectedPhysical)
}
