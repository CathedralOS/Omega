//! Keep the exact selected handler available for an installed interrupt.
//!
//! This admission authorizes emission of its private code entry. Publishing a
//! table and executing the installation helpers still require separate backing,
//! live-arrival and physical-authority admission; these rows grant none of them.

use std::collections::{BTreeMap, BTreeSet};

use diagnostics::Diagnostic;
use terminal_psi::OperationKind;

use super::{NativeRealizationInput, ProviderAdmissionRequest};
use crate::{AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry};

mod arrival_helpers;
mod configuration;
mod scopes;
mod settlement;
pub(super) use settlement::settle;

/// Check semantic token associations before the separate physical installation
/// gate. Success here grants neither root memory nor permission to publish IDTR.
pub(super) fn require_helper_associations(
    input: &NativeRealizationInput<'_>,
    request: &ProviderAdmissionRequest<'_, '_>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    entry_custody: &super::callback_entries::PreparedEntries<'_>,
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
) -> Result<Vec<crate::AdmittedInstalledInterruptHelper>, Vec<Diagnostic>> {
    let mut operations: Vec<_> = input
        .plan()
        .operations()
        .filter_map(|row| row.psi_operation())
        .collect();
    operations.sort();
    operations.dedup();
    let uses = if input.module().installed_interrupt_registrations.is_empty() {
        Vec::new()
    } else {
        let calls = super::selected_synchronous_call_graph(
            artifact.view(),
            input.target_machine_selections(),
            None,
            installation,
            request.profile,
        )
        .map_err(|reason| {
            super::diagnostics::realization_error("installed arrival call graph", reason)
        })?;
        arrival_helpers::validate(input, request, entry_custody, &calls, &operations).map_err(
            |reason| {
                vec![Diagnostic::error(format!(
                    "installed interrupt arrival: {reason}"
                ))]
            },
        )?
    };
    let covered: BTreeSet<_> = uses.iter().map(|row| row.operation).collect();
    for operation in input
        .module()
        .operations()
        .filter(|operation| operations.binary_search(&operation.id).is_ok())
    {
        let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            continue;
        };
        let kind = input
            .module()
            .boundary_machine(boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
            .and_then(|declaration| declaration.execution.static_interrupt_operation());
        if kind.is_some_and(|kind| {
            kind != language_core::static_interrupts::StaticInterruptOperation::Install
        }) && !covered.contains(&operation.id)
        {
            return Err(vec![Diagnostic::error(format!(
                "installed interrupt arrival: helper {:?} has no exact registered arrival association",
                operation.id
            ))]);
        }
    }
    Ok(uses
        .into_iter()
        .map(|row| crate::AdmittedInstalledInterruptHelper {
            installation: row.installation,
            callback_application: row.callback_application,
            operation: row.operation,
            kind: row.kind,
        })
        .collect())
}

pub(super) fn admit(
    input: &NativeRealizationInput<'_>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
    helpers: Vec<crate::AdmittedInstalledInterruptHelper>,
    entry_custody: &super::callback_entries::PreparedEntries<'_>,
) -> Result<AdmittedInstalledInterruptCohort, Vec<Diagnostic>> {
    let fail = |reason: String| {
        vec![Diagnostic::error(format!(
            "installed interrupt entry: {reason}"
        ))]
    };
    let module = input.module();
    if input.plan().psi != artifact.view().identity() {
        return Err(fail(
            "entry proposal belongs to another Terminal program".into(),
        ));
    }
    let reached: BTreeSet<_> = input
        .plan()
        .operations()
        .filter_map(|op| op.psi_operation())
        .collect();
    // Private bodies keep the whole retained roster, but only their reached
    // installations demand entries. Avoid indexing catalogs for unrelated code.
    if helpers.is_empty()
        && request.installed_interrupts.is_empty()
        && !module
            .installed_interrupt_registrations
            .iter()
            .any(|row| reached.contains(&row.operation))
    {
        return Ok(AdmittedInstalledInterruptCohort::empty(input.plan().psi));
    }
    let mut operations = module
        .operations()
        .map(|operation| (operation.id, operation))
        .collect::<Vec<_>>();
    operations.sort_unstable_by_key(|(id, _)| *id);
    if operations.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(fail("Terminal operation identity is duplicated".into()));
    }
    let mut registrations = BTreeMap::new();
    let mut callbacks = BTreeMap::new();
    for registration in &module.installed_interrupt_registrations {
        if registrations
            .insert(registration.operation, registration)
            .is_some()
        {
            return Err(fail("checked installation occurrence is duplicated".into()));
        }
        for callback in &registration.callbacks {
            if callbacks
                .insert(
                    (registration.operation, callback.application_index),
                    callback,
                )
                .is_some()
            {
                return Err(fail("checked installation callback is duplicated".into()));
            }
        }
    }
    let mut admitted = Vec::new();
    let mut applications = BTreeSet::new();
    let mut placements = BTreeSet::new();
    for proposal in request.installed_interrupts {
        let check = || -> Result<crate::AdmittedCallbackEntry, String> {
            let registration = registrations
                .get(&proposal.operation)
                .ok_or("entry has no checked installation occurrence")?;
            let operation = operations
                .binary_search_by_key(&proposal.operation, |(id, _)| *id)
                .ok()
                .map(|position| operations[position].1)
                .ok_or("installation has no Terminal operation")?;
            if !matches!(&operation.kind, OperationKind::BoundaryCall { boundary, .. }
                if *boundary == registration.installer)
            {
                return Err("installation changed its checked boundary occurrence".into());
            }
            let callback = callbacks
                .get(&(proposal.operation, proposal.placement.application_index))
                .ok_or("entry is absent from the checked installation roster")?;
            if !callback.entry_admitted
                || callback.registrar != proposal.operation
                || callback.static_machine_ordinal != proposal.placement.static_machine_ordinal
                || callback.requirement != proposal.placement.canonical_requirement_overload
                || callback.body.contract != proposal.placement.contract_commitment
            {
                return Err("entry changed its checked callback contract".into());
            }
            if proposal.callback_function.caller() != input.plan().psi
                || proposal.callback_function.callback_thunk_placement_index()
                    != Some(proposal.placement_index)
            {
                return Err("entry changed its program or private placement identity".into());
            }
            let occurrence = entry_custody.occurrence(proposal.placement_index)?;
            if occurrence.application_index() != proposal.placement.application_index
                || occurrence.terminal_operation() != proposal.operation
                || occurrence.callback_thunk_identity() != proposal.callback_function
                || *occurrence.boundary_entry_plan() != proposal.placement.boundary_entry_plan
                || occurrence.entry_parameters() != proposal.placement.entry_parameters.as_slice()
            {
                return Err("entry changed its retained body/placement association".into());
            }
            let thunk = occurrence.callback_thunk_artifact();
            // The private body is re-admitted under its own entry. Its original
            // installation need not execute inside that body, but the complete
            // occurrence join above still binds this exact application and plan.
            if !reached.contains(&proposal.operation) && thunk.entry() != input.plan().entry {
                return Err(
                    "entry is neither at a reached installation nor this exact private body".into(),
                );
            }
            let admitted_entry = entry_custody.admit(callback, occurrence)?;
            let registrar = entry_custody.selected(
                registration.registrar_boundary,
                &registration.registrar_application,
            )?;
            let materialization = proposal
                .placement
                .private_materialization
                .as_ref()
                .ok_or("entry lost its private address destination")?;
            if materialization.registrar_application != *registrar
                || materialization.registrar_application_commitment != registrar.commitment
                || materialization.registrar_boundary_entry_plan != *registrar.plan.plan()
            {
                return Err("entry changed its independently selected registrar plan".into());
            }
            let registrar_boundary = super::super::selected_calling_boundary(
                module,
                input.target_machine_selections(),
                registration.registrar_boundary,
            )?;
            let registrar_parameters =
                calling_conventions::TerminalCallingTelescope::for_application(
                    module,
                    registrar_boundary,
                    registrar,
                )?;
            let mut binders = registrar_parameters.binders.iter().filter(|row| {
                row.static_machine_ordinal == callback.static_machine_ordinal
                    && row.requirement_identity == callback.requirement
            });
            let (Some(binder), None) = (binders.next(), binders.next()) else {
                return Err("entry has no unique declared registrar binder".into());
            };
            if binder.binder != materialization.binder
                || binder.requirement != materialization.requirement
                || binder.invokes_synchronously != materialization.registrar_invokes_callback
            {
                return Err(
                    "entry changed its declared registrar binder or invocation contract".into(),
                );
            }
            let expected = registrar.authenticated_callback_placements(
                registrar.commitment,
                materialization.binder,
                &materialization.destination,
                materialization.destination_application.application(),
                &materialization.context,
            )?;
            if expected.as_deref()
                != materialization
                    .destination_application
                    .placements()
                    .map(|rows| rows.as_slice())
            {
                return Err("entry changed its selected private address geometry".into());
            }
            Ok(admitted_entry)
        };
        let admitted_entry = check().map_err(fail)?;
        if !applications.insert(proposal.placement.application_index)
            || !placements.insert(proposal.placement_index)
        {
            return Err(fail(
                "entry repeats a checked application or private placement".into(),
            ));
        }
        admitted.push(AdmittedInstalledInterruptEntry::admit(
            proposal.clone(),
            admitted_entry,
        ));
    }
    for registration in &module.installed_interrupt_registrations {
        if reached.contains(&registration.operation)
            && registration
                .callbacks
                .iter()
                .any(|row| !applications.contains(&row.application_index))
        {
            return Err(fail(
                "reached installation is missing a selected handler entry".into(),
            ));
        }
    }
    let scopes = scopes::prepare(
        input,
        artifact,
        request,
        installation,
        &admitted,
        &helpers,
        &operations,
    )
    .map_err(fail)?;
    Ok(AdmittedInstalledInterruptCohort {
        psi: input.plan().psi,
        entries: admitted,
        // These are the guarded operation's inputs, not a successful runtime
        // installation. Publication still belongs to its native implementation.
        scopes,
        helpers,
    })
}
