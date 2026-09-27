//! Complete target lowering after the explicit post-Terminal optimization stage.

mod output;

pub(crate) use output::NativeTargetStageResult;

use crate::native::native_realization::realization_diagnostics::realization_error;
use crate::native::native_realization::realization_request::NativeRealizationRequest;
use abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) fn lower_realization_target_stage(
    program: ValidatedOptimizedAbstractPlan,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<NativeTargetStageResult, Vec<Diagnostic>> {
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        program,
        abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
            target: request.target,
            settlements,
            installation: provider_installation,
            native_callbacks: request.native_callbacks,
        },
    )
    .map_err(|error| realization_error("target lowering", error))?;
    Ok(NativeTargetStageResult::new(target))
}
