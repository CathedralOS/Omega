//! Optimizer module role: executable entrance. Complete physical routing before machine emission.

use crate::realization::callback_machine_code::validate_callback_thunk_assignments;
use crate::realization::diagnostics::{realization_error, selected_physical_pipeline_failed};
use crate::realization::model::NativeRealizationCoreRequest;
use crate::realization::target_stage::{NativeTargetStageEvidence, NativeTargetStageResult};
use diagnostics::Diagnostic;

use target_operations_to_selected_instructions::is_fragment_publication_program;
#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(crate) struct OptimizedNativePhysicalStage {
    pub(crate) physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
    pub(crate) optimized_plan: abstract_operations::AbstractOperationPlan,
    pub(crate) terminal: terminal_psi::TerminalPsiIdentity,
    pub(crate) validation: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(crate) final_unit: optimization_core::OptimizationUnitIdentity,
    pub(crate) has_provider_installation: bool,
}

/// One completed physical-routing stage result.
///
/// Unit returns and supported scalar bodies share the fragment result with
/// selected execution. Richer ordinary and ranked programs still use baseline assignment
/// until their ABI, call and control facts reach the same fragment postcondition.
#[derive(Debug)]
pub(crate) enum NativePhysicalStageResult {
    Assigned(assigned_target_operations::AssignedOperationPlanWithNativeCallbacks),
    Optimized(Box<OptimizedNativePhysicalStage>),
}

pub(crate) fn lower_realization_physical_stage(
    target_stage: NativeTargetStageResult,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativePhysicalStageResult, Vec<Diagnostic>> {
    let (target, evidence) = target_stage
        .into_parts()
        .map_err(|error| realization_error("target program/evidence join", error))?;
    match evidence {
        NativeTargetStageEvidence::Ranked => assign_current_target(&target, request),
        NativeTargetStageEvidence::Ordinary(optimized_target) => {
            // Transitional physical split only. Target production and its
            // retained translation evidence no longer depend on this selection.
            if request.optimization_selections.is_empty()
                && !(is_fragment_publication_program(&optimized_target)
                    && optimized_target.provider_installation().is_none()
                    && request.settlements.is_empty()
                    && request.compiler_builtins.is_empty()
                    && request.native_callbacks.is_empty()
                    && request.callback_thunks.is_empty())
            {
                return assign_current_target(&target, request);
            }
            let optimized_plan = optimized_target.optimized().plan().clone();
            let optimized_validation = optimized_target.optimized().validation();
            let has_provider_installation = optimized_target.provider_installation().is_some();
            let physical = crate::stage_optimized_verified_physical_pipeline(
                *optimized_target,
                request.optimization_selections,
            )
            .map_err(|error| {
                selected_physical_pipeline_failed(
                    request.optimization_selections.selections(),
                    error,
                )
            })?;
            Ok(NativePhysicalStageResult::Optimized(Box::new(
                OptimizedNativePhysicalStage {
                    physical,
                    optimized_plan,
                    terminal: optimized_validation.psi(),
                    validation: optimized_validation.identity(),
                    final_unit: optimized_validation.final_unit(),
                    has_provider_installation,
                },
            )))
        }
    }
}

fn assign_current_target(
    target: &target_operations::TargetOperationPlanWithNativeCallbacks,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativePhysicalStageResult, Vec<Diagnostic>> {
    let assigned =
        target_operations_to_assigned_target_operations::assign_registers_with_native_callbacks(
            target,
        )
        .map_err(|error| realization_error("ordinary physical assignment", error))?;
    validate_callback_thunk_assignments(
        request.callback_thunks,
        &assigned.native_callback_arguments,
    )?;
    Ok(NativePhysicalStageResult::Assigned(assigned))
}
