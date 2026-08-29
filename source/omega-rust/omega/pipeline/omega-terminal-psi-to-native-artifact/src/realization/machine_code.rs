use crate::realization::diagnostics::{
    realization_error, selected_physical_pipeline_failed,
    selected_physical_pipeline_not_publishable,
};
use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_machine_code::MachineCodePlan;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

pub(crate) fn emit_realization_machine_code(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    request: &NativeRealizationRequest<'_>,
) -> Result<MachineCodePlan, Vec<Diagnostic>> {
    match input {
        NativeRealizationInput::Ordinary(plan) => {
            let target = match provider_installation {
                Some(installation) => {
                    omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions_and_installation(
                        &plan,
                        request.target,
                        settlements,
                        Some(&installation),
                    )
                }
                None => omega_abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
                    &plan,
                    request.target,
                    settlements,
                ),
            }
            .map_err(|error| realization_error("ordinary target lowering", error))?;
            let assigned =
                omega_target_operations_to_assigned_target_operations::assign_registers(&target)
                    .map_err(|error| realization_error("ordinary physical assignment", error))?;
            omega_machine_emission::emit_machine_code(&assigned)
                .map_err(|error| realization_error("machine-code emission", error))
        }
        NativeRealizationInput::ExplicitOptimization(input) => {
            let optimization_request = omega_optimization_pipeline::compiler_baseline_request_v1(
                request.optimization_selections,
            )
            .map_err(|error| realization_error("canonical optimization request", error))?;
            let optimized = omega_optimization_pipeline::optimize_verified_psi_input(
                input,
                optimization_request,
            )
            .map_err(|error| realization_error("canonical optimization", error))?;
            let continuation = match provider_installation {
                Some(installation) => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions_and_installation(
                    optimized,
                    request.target,
                    settlements,
                    installation,
                ),
                None => omega_optimization_pipeline::stage_optimized_native_continuation_with_provider_executions(
                    optimized,
                    request.target,
                    settlements,
                ),
            }
            .map_err(|error| match error {
                omega_optimization_pipeline::OptimizedNativeContinuationError::CoverageFallbackAssigned(
                    error,
                ) => realization_error("optimized physical assignment", error),
                omega_optimization_pipeline::OptimizedNativeContinuationError::SelectedPhysical(
                    error,
                ) => selected_physical_pipeline_failed(request.optimization_selections, error),
            })?;
            let assigned = match continuation {
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::CoverageFallbackAssigned(
                    assigned,
                ) => assigned,
                omega_optimization_pipeline::StagedOptimizedNativeContinuation::SelectedPhysical(
                    physical,
                ) => {
                    return Err(selected_physical_pipeline_not_publishable(
                        request.optimization_selections,
                        &physical,
                    ));
                }
            };
            omega_machine_emission::emit_machine_code(assigned.assigned())
                .map_err(|error| realization_error("machine-code emission", error))
        }
    }
}
