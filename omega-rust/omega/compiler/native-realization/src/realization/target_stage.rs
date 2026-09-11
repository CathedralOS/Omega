//! Complete target lowering after the explicit post-Terminal optimization stage.

mod output;

pub(crate) use output::NativeTargetStageResult;

use crate::realization::diagnostics::realization_error;
use crate::realization::model::NativeRealizationCoreRequest;
use crate::realization::optimization_stage::NativeOptimizationStageResult;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) fn lower_realization_target_stage(
    optimization_stage: NativeOptimizationStageResult,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeTargetStageResult, Vec<Diagnostic>> {
    let NativeOptimizationStageResult { program } = optimization_stage;
    let target =
        abstract_operations_to_target_operations::lower_validated_abstract_to_target_operations(
            program,
            request.target,
            settlements,
            provider_installation,
            request.ieee_float_fma,
            request.native_callbacks,
        )
        .map_err(|error| realization_error("target lowering", error))?;
    Ok(NativeTargetStageResult::new(target))
}
