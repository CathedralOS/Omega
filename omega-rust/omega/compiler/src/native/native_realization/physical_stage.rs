//! Complete one physical stage sequence before machine emission.

use crate::native::native_realization::realization_diagnostics::{
    realization_error, selected_physical_pipeline_failed,
};
use crate::native::native_realization::realization_request::NativeRealizationRequest;
use crate::native::native_realization::target_stage::NativeTargetStageResult;
use diagnostics::Diagnostic;

pub(crate) fn lower_realization_physical_stage(
    target_stage: NativeTargetStageResult,
    request: &NativeRealizationRequest<'_>,
) -> Result<crate::native::StagedOptimizedVerifiedPhysicalPipeline, Vec<Diagnostic>> {
    let (_, optimized_target) = target_stage
        .into_parts()
        .map_err(|error| realization_error("target program/evidence join", error))?;
    crate::native::stage_optimized_verified_physical_pipeline(
        optimized_target,
        request.optimization_selections,
    )
    .map_err(|error| {
        selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
    })
}
