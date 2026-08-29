use omega_abstract_operations_to_target_operations::{
    AdmittedBoundarySettlement, LoweringError, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
};
use omega_optimization_run_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

pub(super) fn lower_optimized_plan(
    optimized: &ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations(optimized.plan(), target)
}

pub(super) fn lower_optimized_plan_with_provider_executions(
    optimized: &ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_provider_executions(optimized.plan(), target, settlements)
}

pub(super) fn lower_optimized_plan_with_provider_installation(
    optimized: &ValidatedOptimizedAbstractPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: &AdmittedProviderInstallation,
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_provider_executions_and_installation(
        optimized.plan(),
        target,
        settlements,
        Some(installation),
    )
}
