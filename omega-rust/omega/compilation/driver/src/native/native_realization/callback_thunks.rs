//! Lower each settled callback thunk's canonical Terminal artifact to machine
//! code inside the same realization as the semantic program.
//!
//! `produce_callback_thunk_artifact` deliberately stops at
//! `finalize_terminal_artifact`: the settlement carries the artifact, private
//! function identity and boundary entry plan, never machine bytes or the
//! producer's source-to-Terminal lowering receipt. Backend callback admission
//! rechecks that receipt before projecting these native inputs. This stage is
//! the consumer half of that contract — every thunk replays the ordinary
//! native pipeline (decode, abstract optimization, target lowering,
//! instruction selection, register homes, fragment emission, frame
//! application, fixed-frame text placement) as a standalone module and
//! materializes as a `CompilerPrivateMachineCodeFunction` the retained
//! object's private-function channel can publish.

use crate::native::native_realization::realization_diagnostics::{
    artifact_lowering_error, realization_error,
};
use backend_plan::NativeCallbackThunkSettlement;
use compilation_products::NativeRealizationRequest;
use diagnostics::Diagnostic;
use machine_code::CompilerPrivateMachineCodeFunction;

/// Realize every settled callback thunk into its compiler-private machine-code
/// record, preserving the request's placement order.
///
/// A thunk is a self-contained module: it carries no boundary settlements,
/// provider installations or callback arguments of its own, so
/// the shared stages run with empty auxiliary rosters. Any decode, lowering,
/// or shape violation rejects here with the thunk's placement index in the
/// diagnostic rather than surfacing as an unrelated program-stage failure.
pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
) -> Result<Vec<CompilerPrivateMachineCodeFunction>, Vec<Diagnostic>> {
    request
        .callback_thunks
        .iter()
        .map(|settlement| lower_callback_thunk(request, settlement))
        .collect()
}

fn lower_callback_thunk(
    request: &NativeRealizationRequest<'_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
) -> Result<CompilerPrivateMachineCodeFunction, Vec<Diagnostic>> {
    let artifact = settlement.artifact;
    artifact
        .validate()
        .map_err(|error| realization_error("callback thunk artifact replay", error))?;
    let input = terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput::prepare(
        artifact,
        &[],
    )
    .map_err(artifact_lowering_error)?;
    let selections = request.optimization_selections.selections();
    let optimized = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        request.optimization_budget(),
    )
    .map_err(|error| realization_error("callback thunk abstract optimization", error))?;
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
                request.admission.providers.target,
            ),
        )
        .map_err(|error| realization_error("callback thunk target lowering", error))?;
    let physical = crate::native::stage_optimized_verified_physical_pipeline(optimized_target)
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
            private_symbol: settlement.private_symbol,
            boundary_entry_plan: settlement.boundary_entry_plan,
            placement_index: settlement.placement_index,
        },
    )
}
