//! Lower each settled callback thunk's canonical Terminal artifact to machine
//! objects inside the same realization as the semantic program.
//!
//! The settlement carries a Terminal artifact, a caller-scoped private function
//! identity and a boundary entry plan. Every thunk follows the ordinary native
//! pipeline. The object linker retains the complete local closure under the
//! caller-scoped identity, not a synthetic single-function byte projection.

use crate::request::NativeRealizationRequest;
use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use image_emission::PrivateFunctionObject;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;

/// Realize the executable scope's callback thunks into compiler-private code,
/// preserving original placement identities and relative request order.
///
/// Each thunk selects an entry in the shared Terminal namespace. Admission
/// uses the same prepared providers and independent receiving policy without
/// manufacturing a ProgramEntry for the private body.
pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
    native_callbacks: &[terminal_psi_to_abstract_operations::AdmittedNativeCallbackArgument],
    root: &image_emission::ObjectArtifact,
) -> Result<Vec<PrivateFunctionObject>, Vec<Diagnostic>> {
    let mut pending = request
        .callback_thunks
        .iter()
        .filter(|settlement| {
            root.object()
                .layout
                .function_symbols
                .iter()
                .any(|(_, reference)| reference.identity == settlement.callback_function)
                && native_callbacks.iter().any(|callback| {
                    callback.terminal_operation == settlement.terminal_operation
                        && callback.placement_index == settlement.placement_index
                        && callback.callback_function == settlement.callback_function
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut objects = Vec::new();
    let mut next_callback = 0;
    while next_callback < pending.len() {
        let settlement = &pending[next_callback];
        let object = lower_callback_thunk(request, settlement)?;
        for child in request.callback_thunks.iter().copied() {
            if !object
                .body
                .object()
                .layout
                .function_symbols
                .iter()
                .any(|(_, reference)| reference.identity == child.callback_function)
            {
                continue;
            }
            if let Some(known) = pending
                .iter()
                .find(|known| known.callback_function == child.callback_function)
            {
                if known.artifact.manifest().identity() != child.artifact.manifest().identity()
                    || known.entry != child.entry
                    || known.boundary_entry_plan != child.boundary_entry_plan
                    || known.parameter_formals != child.parameter_formals
                    || known.entry_parameters != child.entry_parameters
                {
                    return Err(realization_error(
                        "callback body closure",
                        "one private identity selects incompatible bodies or entry plans",
                    ));
                }
            } else {
                pending.push(child);
            }
        }
        objects.push(object);
        next_callback += 1;
    }
    Ok(objects)
}

fn lower_callback_thunk(
    request: &NativeRealizationRequest<'_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
) -> Result<PrivateFunctionObject, Vec<Diagnostic>> {
    let projection = super::callable_objects::emit_callable_object(
        request.admission,
        request.optimization_selections,
        request.optimization_budget(),
        settlement.artifact,
        terminal_psi::TargetMachineCallable::Machine(settlement.entry),
    )?
    .projection;
    let emitted = projection.entry_call_plan()?;
    let declared = &settlement.boundary_entry_plan.call;
    // Join independent Terminal lanes through their exact nominal origins.
    // The body keeps its ordinary ABI; a differing admitted native recipe is
    // realized by a separate physical entry, not by rewriting helper/recursion
    // conventions or treating a thin scalar pointer as a descriptor.
    if emitted.parameters.len() != declared.parameters.len()
        || emitted.parameters.len() != settlement.parameter_formals.len()
        || settlement.entry_parameters.len() != declared.parameters.len()
    {
        return Err(realization_error(
            "callback entry ABI",
            "callback signature drifts from its boundary entry plan",
        ));
    }
    let inbound_ordinals = settlement
        .parameter_formals
        .iter()
        .map(|formal| {
            let mut matching = settlement.entry_parameters.iter().filter(|parameter| {
                parameter.origin
                    == calling_conventions::BoundaryNativeParameterOrigin::SemanticFormal {
                        formal_ordinal: *formal,
                    }
            });
            let Some(parameter) = matching.next() else {
                return Err(realization_error(
                    "callback entry ABI",
                    "callback formal has no native parameter origin",
                ));
            };
            let boundary = usize::try_from(parameter.application.native_ordinal)
                .ok()
                .and_then(|ordinal| declared.parameters.get(ordinal));
            if matching.next().is_some()
                || boundary.is_none_or(|boundary| {
                    parameter.application.placement != *boundary
                        || parameter.application.shape != boundary.shape
                })
            {
                return Err(realization_error(
                    "callback entry ABI",
                    "callback native parameter application differs",
                ));
            }
            Ok(parameter.application.native_ordinal)
        })
        .collect::<Result<Vec<_>, _>>()?;
    image_emission::create_private_function_object(
        settlement.callback_function,
        std::sync::Arc::new(projection.into_object()),
        declared,
        &inbound_ordinals,
    )
    .map_err(|error| realization_error("callback entry ABI", error))
}
