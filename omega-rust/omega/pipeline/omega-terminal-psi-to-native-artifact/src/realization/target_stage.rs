//! Optimizer module role: executable entrance. Complete target lowering before physical routing.

use crate::realization::diagnostics::realization_error;
use crate::realization::model::{
    NativeRealizationCoreRequest, NativeRealizationInput, PostTerminalOptimizationContinuation,
};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderInstallationEvidence;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

/// One completed target-lowering stage result.
///
/// The variants retain authority-distinct payloads rather than using those
/// differences to bypass the stage. Physical assignment and optimization
/// consume this result and never inspect the earlier continuation selection.
#[derive(Debug)]
pub(crate) enum NativeTargetStageResult {
    IdentityOrdinary(omega_target_operations::TargetOperationPlanWithNativeCallbacks),
    IdentityRanked(omega_target_operations::TargetOperationPlan),
    Selected(Box<omega_optimization_pipeline::ValidatedOptimizedTargetOperations>),
}

pub(crate) fn lower_realization_target_stage(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeTargetStageResult, Vec<Diagnostic>> {
    match input.into_parts() {
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(plan),
            PostTerminalOptimizationContinuation::Identity,
        ) => {
            let installation = provider_installation
                .as_ref()
                .map(|installation| installation as &dyn ProviderInstallationEvidence);
            let target =
                omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
                    &plan,
                    request.target,
                    settlements,
                    installation,
                    request.ieee_float_fma,
                    request.native_callbacks,
                )
                .map_err(|error| realization_error("ordinary target lowering", error))?;
            Ok(NativeTargetStageResult::IdentityOrdinary(target))
        }
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
                ranked,
            ),
            PostTerminalOptimizationContinuation::Identity,
        ) => {
            if provider_installation.is_some()
                || !settlements.is_empty()
                || !request.native_callbacks.is_empty()
                || !request.callback_thunks.is_empty()
            {
                return Err(realization_error(
                    "ranked native provider isolation",
                    "the exact ranked countdown admits no provider installation or boundary settlement",
                ));
            }
            let target =
                omega_abstract_operations_to_target_operations::lower_ranked_to_target_operations(
                    &ranked,
                    request.target,
                )
                .map_err(|error| realization_error("ranked target lowering", error))?;
            Ok(NativeTargetStageResult::IdentityRanked(target))
        }
        (_, PostTerminalOptimizationContinuation::Selected(input)) => {
            if !request.native_callbacks.is_empty() || !request.callback_thunks.is_empty() {
                return Err(realization_error(
                    "optimized native callback custody",
                    "retained callbacks require the ordinary custody-preserving pipeline",
                ));
            }
            if !request.ieee_float_fma.is_empty() {
                return Err(realization_error(
                    "optimized nearest-FMA custody",
                    "retained nearest-FMA occurrences require the ordinary custody-preserving pipeline",
                ));
            }
            let optimization_request = omega_optimization_pipeline::compiler_baseline_request_v1(
                request.optimization_selections.selections(),
            );
            let optimized = omega_optimization_pipeline::optimize_verified_psi_input(
                input,
                optimization_request,
            )
            .map_err(|error| realization_error("canonical optimization", error))?;
            let optimized_target = match provider_installation {
                Some(installation) => omega_optimization_pipeline::lower_optimized_to_target_operations_with_provider_executions_and_installation(
                    optimized,
                    request.target,
                    settlements,
                    installation,
                ),
                None => omega_optimization_pipeline::lower_optimized_to_target_operations_with_provider_executions(
                    optimized,
                    request.target,
                    settlements,
                ),
            }
            .map_err(|error| realization_error("optimized target lowering", error))?;
            Ok(NativeTargetStageResult::Selected(Box::new(
                optimized_target,
            )))
        }
    }
}
