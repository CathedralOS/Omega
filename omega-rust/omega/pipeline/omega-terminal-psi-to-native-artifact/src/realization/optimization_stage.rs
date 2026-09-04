//! Complete the explicit post-Terminal optimization continuation before target lowering.

use crate::realization::diagnostics::realization_error;
use crate::realization::model::{
    NativeRealizationCoreRequest, NativeRealizationInput, PostTerminalOptimizationContinuation,
};
use psi_diagnostics::Diagnostic;

/// One completed post-Terminal optimization stage.
///
/// Identity execution preserves the independently admitted ordinary or ranked
/// native authority. Selected execution is currently admitted only for the
/// ordinary authority role; ranked selection rejects instead of substituting
/// the ordinary optimized route.
#[derive(Debug)]
pub(crate) enum NativeOptimizationStageResult {
    IdentityOrdinary(omega_abstract_operations::AbstractOperationPlan),
    IdentityRanked(omega_abstract_operations::RankedNativeAbstractOperationPlan),
    OptimizedOrdinary(omega_optimization_pipeline::ValidatedOptimizedAbstractPlan),
}

pub(crate) fn lower_realization_optimization_stage(
    input: NativeRealizationInput,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeOptimizationStageResult, Vec<Diagnostic>> {
    match input.into_parts() {
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(plan),
            PostTerminalOptimizationContinuation::Identity,
        ) => Ok(NativeOptimizationStageResult::IdentityOrdinary(plan)),
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(
                ranked,
            ),
            PostTerminalOptimizationContinuation::Identity,
        ) => Ok(NativeOptimizationStageResult::IdentityRanked(ranked)),
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::RankedU32Countdown(_),
            PostTerminalOptimizationContinuation::Selected(_),
        ) => Err(realization_error(
            "optimized ranked-native authority",
            "the selected optimizer route does not yet retain ranked-countdown native authority; no ordinary optimized route was substituted",
        )),
        (
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(_),
            PostTerminalOptimizationContinuation::Selected(input),
        ) => {
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
            omega_optimization_pipeline::optimize_verified_psi_input(input, optimization_request)
                .map(NativeOptimizationStageResult::OptimizedOrdinary)
                .map_err(|error| realization_error("canonical optimization", error))
        }
    }
}
