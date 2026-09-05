//! Complete target lowering after the explicit post-Terminal optimization stage.

mod output;

pub(crate) use output::{NativeTargetStageEvidence, NativeTargetStageResult};

use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationAuthority, NativeRealizationCoreRequest};
use crate::realization::optimization_stage::NativeOptimizationStageResult;
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_installation_evidence::ProviderInstallationEvidence;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

pub(crate) fn lower_realization_target_stage(
    optimization_stage: NativeOptimizationStageResult,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeTargetStageResult, Vec<Diagnostic>> {
    let NativeOptimizationStageResult { program, authority } = optimization_stage;
    match authority {
        NativeRealizationAuthority::Ordinary if request.optimization_selections.is_empty() => {
            let identity = program;
            let installation = provider_installation
                .as_ref()
                .map(|installation| installation as &dyn ProviderInstallationEvidence);
            let target =
                omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
                    identity.plan(),
                    request.target,
                    settlements,
                    installation,
                    request.ieee_float_fma,
                    request.native_callbacks,
                )
                .map_err(|error| realization_error("ordinary target lowering", error))?;
            Ok(NativeTargetStageResult::ordinary(target))
        }
        NativeRealizationAuthority::RankedU32Countdown(ranked) => {
            let abstract_identity = program;
            if abstract_identity.plan() != &ranked.plan {
                return Err(realization_error(
                    "ranked abstract optimization identity",
                    "the validated identity result no longer matches ranked native authority",
                ));
            }
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
            Ok(NativeTargetStageResult::ranked(target))
        }
        NativeRealizationAuthority::Ordinary => {
            let optimized = program;
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
            Ok(NativeTargetStageResult::optimized(optimized_target))
        }
    }
}
