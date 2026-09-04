//! Optimizer module role: executable entrance. Complete physical routing before machine emission.

use crate::realization::callback_machine_code::validate_callback_thunk_assignments;
use crate::realization::diagnostics::{realization_error, selected_physical_pipeline_failed};
use crate::realization::model::NativeRealizationCoreRequest;
use crate::realization::target_stage::NativeTargetStageResult;
use psi_diagnostics::Diagnostic;

#[derive(Debug)]
pub(crate) struct SelectedNativePhysicalStage {
    pub(crate) physical: omega_optimization_pipeline::StagedOptimizedVerifiedPhysicalPipeline,
    pub(crate) optimized_plan: omega_abstract_operations::AbstractOperationPlan,
    pub(crate) terminal: psi_terminal::TerminalPsiIdentity,
    pub(crate) validation: omega_optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(crate) final_unit: omega_optimization_core::OptimizationUnitIdentity,
    pub(crate) has_provider_installation: bool,
}

/// One completed physical-routing stage result.
///
/// Identity assignment and selected physical optimization enter through the
/// same stage. Their authority-distinct results remain explicit for emission;
/// neither can fall back to the other.
#[derive(Debug)]
pub(crate) enum NativePhysicalStageResult {
    IdentityOrdinary(omega_assigned_target_operations::AssignedOperationPlanWithNativeCallbacks),
    IdentityRanked(omega_assigned_target_operations::AssignedOperationPlan),
    Selected(Box<SelectedNativePhysicalStage>),
}

pub(crate) fn lower_realization_physical_stage(
    target_stage: NativeTargetStageResult,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativePhysicalStageResult, Vec<Diagnostic>> {
    match target_stage {
        NativeTargetStageResult::IdentityOrdinary(target) => {
            let assigned = omega_target_operations_to_assigned_target_operations::assign_registers_with_native_callbacks(&target)
                .map_err(|error| realization_error("ordinary physical assignment", error))?;
            validate_callback_thunk_assignments(
                request.callback_thunks,
                &assigned.native_callback_arguments,
            )?;
            Ok(NativePhysicalStageResult::IdentityOrdinary(assigned))
        }
        NativeTargetStageResult::IdentityRanked(target) => {
            let assigned =
                omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                    .map_err(|error| realization_error("ranked physical assignment", error))?;
            Ok(NativePhysicalStageResult::IdentityRanked(assigned))
        }
        NativeTargetStageResult::Selected(optimized_target) => {
            let optimized_plan = optimized_target.optimized().plan().clone();
            let optimized_validation = optimized_target.optimized().validation();
            let has_provider_installation = optimized_target.provider_installation().is_some();
            let physical = omega_optimization_pipeline::stage_optimized_verified_physical_pipeline(
                *optimized_target,
                request.optimization_selections,
            )
            .map_err(|error| {
                selected_physical_pipeline_failed(
                    request.optimization_selections.selections(),
                    error,
                )
            })?;
            Ok(NativePhysicalStageResult::Selected(Box::new(
                SelectedNativePhysicalStage {
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
