//! Optimizer module role: executable entrance. Complete physical routing before machine emission.

use crate::realization::callback_machine_code::validate_callback_thunk_assignments;
use crate::realization::diagnostics::{realization_error, selected_physical_pipeline_failed};
use crate::realization::model::NativeRealizationCoreRequest;
use crate::realization::target_stage::{NativeTargetStageEvidence, NativeTargetStageResult};
use diagnostics::Diagnostic;

mod fragment_shape;
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
                && !(fragment_program(&optimized_target)
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

/// Shapes whose complete ABI and publication already use the fragment path.
/// This migration boundary is chosen from the current program, never by
/// trying the new implementation and falling back after an error.
fn return_only_fragment_program(plan: &abstract_operations::AbstractOperationPlan) -> bool {
    !plan.functions.is_empty()
        && plan.functions.iter().all(|function| {
            function.parameters.is_empty()
                && function.structural_parameters.is_empty()
                && matches!(
                    function.result,
                    abstract_operations::AbstractFunctionResult::Unit
                )
                && matches!(function.block_entries.as_slice(), [block]
                    if block.block == function.entry && block.operation_offset == 0
                        && block.parameters.is_empty())
                && matches!(
                    function.operations.as_slice(),
                    [abstract_operations::AbstractOperation::ReturnUnit { cleanup_actions, .. }]
                        if cleanup_actions.is_empty()
                )
        })
}

fn fragment_program(
    target: &abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations,
) -> bool {
    let plan = target.optimized().plan();
    if return_only_fragment_program(plan) {
        return true;
    }
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .expect("u64"),
    );
    !plan.functions.is_empty() && plan.functions.iter().all(|function| {
        let Some(native) = target
            .target_operations()
            .functions
            .iter()
            .find(|native| native.machine == function.machine)
        else {
            return false;
        };
        if fragment_shape::scalar_conditional(function, native) {
            return true;
        }
        function.attachment.is_none()
            && function.structural_parameters.is_empty()
            && function.entry_claims.is_empty()
            && function.published_service_ceiling.is_empty()
            && function
                .parameters
                .iter()
                .all(|parameter| parameter.scalar_type == scalar_type)
            && matches!(function.result, abstract_operations::AbstractFunctionResult::Scalar(result)
                if result.scalar_type == scalar_type)
            && matches!(function.block_entries.as_slice(), [block]
                if block.block == function.entry && block.operation_offset == 0
                    && block.parameters.is_empty())
            && matches!(function.operations.as_slice(),
                [abstract_operations::AbstractOperation::Return { cleanup_actions, .. }]
                | [abstract_operations::AbstractOperation::IntegerConstant { .. },
                   abstract_operations::AbstractOperation::Return { cleanup_actions, .. }]
                if cleanup_actions.is_empty())
            && matches!(
                native.operation,
                target_operations::TargetOperation::ReturnIntegerImmediate { .. }
                    | target_operations::TargetOperation::ReturnIntegerParameter {
                        location: target_operations::ScalarParameterLocation::Register(_),
                        ..
                    }
            )
    })
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
