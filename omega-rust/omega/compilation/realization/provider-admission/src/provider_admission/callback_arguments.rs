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
pub fn project_native_callback_arguments(
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
        // The retained placement owns the registrar telescope row its
        // destination selected; the occurrence only names that placement.
        let application = materialization.destination_application.application();
        admitted.push(crate::NativeCallbackArgumentProposal {
            registrar_invokes_callback: materialization.registrar_invokes_callback,
            terminal_operation: occurrence.terminal_operation(),
            placement_index: occurrence.placement_index(),
            callback_function: occurrence.callback_thunk_identity(),
            destination: materialization.destination.clone(),
            field_placements: materialization
                .destination_application
                .placements()
                .cloned(),
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

/// Freeze the entry recipe only after ordinary registrar admission succeeded.
/// The expected recipe comes from the original occurrence and Calling roster.
pub(super) fn admit_entries(
    input: &super::NativeRealizationInput<'_>,
    request: &super::ProviderAdmissionRequest<'_, '_>,
    entries: &super::callback_entries::PreparedEntries<'_>,
) -> Result<Vec<super::AdmittedNativeCallbackArgument>, String> {
    request
        .native_callbacks
        .iter()
        .map(|proposal| {
            let occurrence = entries.occurrence(proposal.placement_index)?;
            if occurrence.terminal_operation() != proposal.terminal_operation
                || occurrence.callback_thunk_identity() != proposal.callback_function
            {
                return Err("callback argument changed its original entry occurrence".into());
            }
            let callback = input
                .module()
                .callback_application(occurrence.application_index())
                .ok_or("callback entry lost its checked application")?;
            let entry = entries.admit(callback, occurrence)?;
            Ok(super::AdmittedNativeCallbackArgument::admit(
                proposal.clone(),
                entry,
            ))
        })
        .collect()
}
