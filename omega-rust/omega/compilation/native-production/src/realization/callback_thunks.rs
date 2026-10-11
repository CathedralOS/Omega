//! Lower each settled callback thunk's canonical Terminal artifact to machine
//! code inside the same realization as the semantic program.
//!
//! The settlement carries a Terminal artifact, a caller-scoped private function
//! identity and a boundary entry plan. Every thunk follows the ordinary native
//! pipeline; emission derives its linker name from the retained identity.

use crate::request::NativeRealizationRequest;
use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use machine_code::CompilerPrivateMachineCodeFunction;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

/// Realize the executable scope's callback thunks into compiler-private code,
/// preserving original placement identities and relative request order.
///
/// Each thunk owns its own Terminal namespace. Its selected providers and
/// native occurrences were closed against that artifact, not copied from the
/// application. Admission uses the same independent receiving policy without
/// manufacturing a ProgramEntry for the private body.
pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
    native_callbacks: &[terminal_psi_to_abstract_operations::AdmittedNativeCallbackArgument],
) -> Result<Vec<CompilerPrivateMachineCodeFunction>, Vec<Diagnostic>> {
    request
        .callback_thunks
        .iter()
        .filter(|settlement| {
            native_callbacks.iter().any(|callback| {
                callback.terminal_operation == settlement.terminal_operation
                    && callback.placement_index == settlement.placement_index
                    && callback.callback_function == settlement.callback_function
            })
        })
        .map(|settlement| lower_callback_thunk(request, settlement))
        .collect()
}

fn lower_callback_thunk(
    request: &NativeRealizationRequest<'_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
) -> Result<CompilerPrivateMachineCodeFunction, Vec<Diagnostic>> {
    let artifact = settlement.artifact;
    let mut admitted = request.callback_providers.admit_callback_body(
        artifact,
        request.admission.providers.terminal_authority_policy,
        request
            .admission
            .providers
            .terminal_authority_permission_policy,
    )?;
    let selections = request.optimization_selections.selections();
    let optimized = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        admitted.input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        request.optimization_budget(),
    )
    .map_err(|error| realization_error("callback thunk abstract optimization", error))?;
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest {
                target: request.admission.providers.target(),
                settlements: &admitted.providers.settlements,
                installation: admitted.providers.installation.take(),
                native_callbacks: &admitted.providers.native_callbacks,
            },
        )
        .map_err(|error| realization_error("callback thunk target lowering", error))?;
    let physical =
        crate::physical_pipeline::stage_optimized_verified_physical_pipeline(optimized_target)
            .map_err(|error| realization_error("callback thunk physical pipeline", error))?;
    let emitted = machine_emission::stage_optimized_function_fragment_emission(physical)
        .map_err(|error| realization_error("callback thunk fragment emission", error))?;
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .map_err(|error| realization_error("callback thunk frame application", error))?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| realization_error("callback thunk text placement", error))?;
    machine_emission::materialize_private_callback_function(
        text,
        machine_emission::PrivateCallbackMaterializationRequest {
            callback_function: settlement.callback_function,
            boundary_entry_plan: settlement.boundary_entry_plan,
            placement_index: settlement.placement_index,
        },
    )
}
