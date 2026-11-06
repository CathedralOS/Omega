//! Bind each checked installation request to its emitted operation.
//!
//! Registrar metadata belongs to this installation occurrence, not to another
//! caller that happens to select the same handler or table type.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_psi::{
    CheckedBoundaryExecution, OperationKind, TerminalInstalledInterruptRegistration,
};

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
    lowered: &mut LoweredPsi,
) -> Result<(), String> {
    let mut registrations = Vec::new();
    for request in &checked.facts.installed_interrupt_registrations {
        let mut occurrences = lowered
            .source_call_occurrences
            .iter()
            .filter_map(|occurrence| {
                if occurrence.source_site != Some(request.site) {
                    return None;
                }
                let operation = occurrence.terminal_site.operation()?;
                lowered.semantic_module.operations().find_map(|row| {
                    if row.id != operation {
                        return None;
                    }
                    let OperationKind::BoundaryCall { boundary, .. } = row.kind else {
                        return None;
                    };
                    let declaration = lowered
                        .semantic_module
                        .boundary_machine(boundary)?
                        .declaration
                        .as_ref()?;
                    matches!(
                        declaration.execution,
                        CheckedBoundaryExecution::StaticInterruptInstall
                    )
                    .then_some((operation, boundary))
                })
            });
        // This source declaration can be outside the requested module closure.
        let Some((operation, installer)) = occurrences.next() else {
            continue;
        };
        if occurrences.next().is_some() {
            return Err("interrupt installation request names multiple emitted operations".into());
        }
        let mut registrar_boundaries = lowered
            .source_boundary_declarations
            .iter()
            .filter(|(source, _)| *source == request.target);
        let registrar_boundary = match (registrar_boundaries.next(), registrar_boundaries.next()) {
            (Some((_, boundary)), None) => *boundary,
            _ => {
                return Err(
                    "interrupt registrar has no unique exact emitted boundary declaration".into(),
                );
            }
        };
        let closed = checked
            .closed_requirement_application(request.target)
            .ok_or("interrupt installation lost its checked registrar application")?;
        let owner = checked
            .trait_definition_by_symbol(closed.declaring_trait)
            .ok_or("interrupt installation registrar lost its declaring trait")?;
        let registrar_requirement =
            checked.trait_requirement_callable_identity_text(owner, &closed.signature);
        let callbacks = request
            .callbacks
            .uses
            .iter()
            .map(|usage| {
                super::callback_applications::lower_application(
                    identities, checked, sources, lowered, usage, operation,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        registrations.push(TerminalInstalledInterruptRegistration {
            operation,
            installer,
            service: emitted_control_service(
                checked,
                identities,
                &lowered.semantic_module,
                request.control_service,
            )?,
            registrar_boundary,
            registrar: super::declarations::nominal(identities, checked, sources, closed.template)
                .map_err(str::to_owned)?,
            registrar_owner: super::declarations::nominal(
                identities,
                checked,
                sources,
                request.registrar_owner,
            )
            .map_err(str::to_owned)?,
            registrar_application: request.registrar_application_identity.clone(),
            registrar_declaring_application: request.registrar_declaring_application.clone(),
            registrar_arguments: request.registrar_arguments.clone(),
            registrar_lifetimes: request.registrar_lifetimes.clone(),
            registrar_requirement,
            slot_declaration: super::declarations::nominal(
                identities,
                checked,
                sources,
                request.slot_application.declaration,
            )
            .map_err(str::to_owned)?,
            slot_commitment: request.slot_application.commitment.as_bytes(),
            slot_encoding: request.slot_application.identity_encoding.to_vec(),
            callbacks,
        });
    }
    registrations.sort_unstable_by_key(|row| row.operation);
    if registrations
        .windows(2)
        .any(|rows| rows[0].operation == rows[1].operation)
    {
        return Err("interrupt installation operation has competing checked requests".into());
    }
    if std::env::var_os("OMEGA_TRACE_NATIVE_BOUNDARY").is_some() {
        for registration in &registrations {
            let helpers = lowered
                .semantic_module
                .callback_applications
                .iter()
                .filter(|application| application.registrar == registration.operation)
                .map(|application| {
                    (
                        application.static_machine_ordinal,
                        application.entry_admitted,
                        application.body.callable,
                    )
                })
                .collect::<Vec<_>>();
            eprintln!(
                "published installed operation {:?} helpers: {helpers:?}",
                registration.operation
            );
        }
    }
    lowered.semantic_module.installed_interrupt_registrations = registrations;
    Ok(())
}

/// Map the checked service selection to its exact emitted declaration.
fn emitted_control_service(
    checked: &CheckedTrees,
    identities: &terminal_psi::TerminalIdentityTableBuilder,
    module: &terminal_psi::TerminalModule,
    selected: language_semantics::ServiceReachId,
) -> Result<semantic_vocabulary::ServiceId, String> {
    let definition = checked
        .facts
        .service_reaches
        .services
        .definition(selected)
        .ok_or("interrupt installation lost its checked control service")?;
    let identity = crate::shared::terminal_identities::service_declaration_identity(
        checked,
        definition.symbol,
    )
    .map_err(|error| format!("interrupt installer service identity: {error:?}"))?;
    let mut emitted = module
        .services
        .iter()
        .filter(|service| identities.text(service.identity) == Some(identity.as_str()));
    match (emitted.next(), emitted.next()) {
        (Some(service), None) => Ok(service.id),
        _ => Err("interrupt installer control service has no unique emitted declaration".into()),
    }
}
