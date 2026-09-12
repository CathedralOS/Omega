//! Complete one physical stage sequence before machine emission.

use crate::realization::diagnostics::{realization_error, selected_physical_pipeline_failed};
use crate::realization::model::NativeRealizationRequest;
use crate::realization::target_stage::NativeTargetStageResult;
use diagnostics::Diagnostic;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(crate) struct NativePhysicalStageResult {
    pub(crate) physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
    pub(crate) optimized_plan: abstract_operations::AbstractOperationPlan,
    pub(crate) terminal: terminal_psi::TerminalPsiIdentity,
    pub(crate) validation: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(crate) final_unit: optimization_core::OptimizationUnitIdentity,
}

pub(crate) fn lower_realization_physical_stage(
    target_stage: NativeTargetStageResult,
    request: &NativeRealizationRequest<'_>,
) -> Result<NativePhysicalStageResult, Vec<Diagnostic>> {
    let (_, optimized_target) = target_stage
        .into_parts()
        .map_err(|error| realization_error("target program/evidence join", error))?;
    let optimized_plan = optimized_target.optimized().plan().clone();
    let optimized_validation = optimized_target.optimized().validation();
    let physical = crate::stage_optimized_verified_physical_pipeline(
        optimized_target,
        request.optimization_selections,
    )
    .map_err(|error| {
        selected_physical_pipeline_failed(request.optimization_selections.selections(), error)
    })?;
    Ok(NativePhysicalStageResult {
        physical,
        optimized_plan,
        terminal: optimized_validation.psi(),
        validation: optimized_validation.identity(),
        final_unit: optimized_validation.final_unit(),
    })
}
