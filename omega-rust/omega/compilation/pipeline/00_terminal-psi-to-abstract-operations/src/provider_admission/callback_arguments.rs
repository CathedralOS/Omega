//! Project retained callback destinations into this stage's native inputs.
//!
//! The retained placement owns the exact registrar plan and application
//! commitment. Admission checks uniqueness and the occurrence-to-destination
//! join; lowering independently checks the projected plan against its target
//! and abstract boundary operation.

use diagnostics::Diagnostic;

/// Project callback arguments in occurrence order. Direct parameters and private
/// fields retain their distinct destinations and the exact registrar context.
/// `None` means at least one callback has no target materialization; Terminal
/// production remains valid, but native admission must reject that proposal.
/// This does not validate the thunk body, admit registration, or replace the
/// target lowerer's independent application replay.
pub(crate) fn project_native_callback_arguments(
    placements: &[backend_plan::NativeCallbackPlacement],
    occurrences: &[backend_plan::TerminalCallbackOccurrenceProposal],
) -> Result<Option<Vec<crate::NativeCallbackArgumentProposal>>, Vec<Diagnostic>> {
    let mut admitted = Vec::with_capacity(occurrences.len());
    let mut admitted_placements = std::collections::BTreeSet::new();
    for occurrence in occurrences {
        if !admitted_placements.insert(occurrence.placement_index()) {
            return Err(callback_error(
                "two callback occurrences share one retained placement",
            ));
        }
        let placement = placements
            .get(occurrence.placement_index())
            .ok_or_else(|| {
                callback_error("callback occurrence names an absent retained placement")
            })?;
        let Some(materialization) = placement.private_materialization.as_ref() else {
            return Ok(None);
        };
        let application = materialization
            .destination_parameter_application()
            .map_err(|error| callback_error(error.0))?;
        if occurrence.destination_parameter_application() != Some(application) {
            return Err(callback_error(
                "callback occurrence parameter application drifted from its retained registrar materialization",
            ));
        }
        admitted.push(crate::NativeCallbackArgumentProposal {
            registrar_invokes_callback: materialization.registrar_invokes_callback,
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            destination: materialization.destination.clone(),
            field_byte_offset: materialization.field_byte_offset,
            application: application.clone(),
            registrar_boundary_entry_plan: materialization.registrar_boundary_entry_plan.clone(),
            registrar_context: materialization.context.clone(),
            registrar_application_commitment: materialization.registrar_application_commitment,
            registrar_application: materialization.registrar_application.clone(),
            binder: materialization.binder,
        });
    }
    Ok(Some(admitted))
}

fn callback_error(message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "Terminal callback custody: {message}"
    ))]
}
