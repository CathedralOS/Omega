//! Check occurrence and identity custody; this grants no native installation.
use super::*;
use terminal_psi::{CheckedBoundaryExecution, OperationKind};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let mut calls = std::collections::BTreeMap::new();
    for operation in module.operations() {
        let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            continue;
        };
        if module
            .boundary_machine(boundary)
            .and_then(|row| row.declaration.as_ref())
            .is_some_and(|row| {
                matches!(
                    row.execution,
                    CheckedBoundaryExecution::StaticInterruptInstall
                )
            })
        {
            calls.insert(operation.id, boundary);
        }
    }
    let mut previous = None;
    let mut callback_ids = module
        .callback_applications
        .iter()
        .map(|row| row.application_index)
        .collect::<std::collections::BTreeSet<_>>();
    for registration in &module.installed_interrupt_registrations {
        if previous.is_some_and(|previous| previous >= registration.operation)
            || calls.remove(&registration.operation) != Some(registration.installer)
        {
            return malformed("interrupt installation has no unique exact core operation");
        }
        previous = Some(registration.operation);
        let installer = module.boundary_machine(registration.installer).ok_or(
            CodecError::MalformedStructuralFoundation("interrupt installation lost its boundary"),
        )?;
        validate_control_service(
            module,
            registration.service,
            &installer.published_service_ceiling,
        )?;
        let Some(registrar) = module
            .boundary_machine(registration.registrar_boundary)
            .and_then(|boundary| boundary.declaration.as_ref())
        else {
            return malformed("interrupt registrar lost its exact boundary declaration");
        };
        if module.identities.text(registrar.source.callable)
            != Some(registration.registrar_requirement.as_str())
        {
            return malformed("interrupt registrar boundary names another checked requirement");
        }
        if registration.registrar_application.is_empty()
            || registration.registrar_declaring_application.is_empty()
            || registration
                .registrar_arguments
                .iter()
                .chain(&registration.registrar_lifetimes)
                .any(String::is_empty)
            || registration.registrar_requirement.is_empty()
            || registration.slot_encoding.is_empty()
            || registration.slot_commitment == [0; 32]
            || language_semantics::conformance_identity::source_identity_commitments(
                &registration.slot_encoding,
            )
            .1 != registration.slot_commitment
        {
            return malformed(
                "interrupt installation lost its complete registrar or slot application",
            );
        }
        for identity in [
            &registration.registrar,
            &registration.registrar_owner,
            &registration.slot_declaration,
        ] {
            if module
                .identities
                .text(identity.identity())
                .is_none_or(str::is_empty)
            {
                return malformed("interrupt installation lost a selected declaration identity");
            }
        }
        if registration.callbacks.is_empty() {
            return malformed("interrupt installation has no checked callback selection");
        }
        let mut previous_callback = None;
        for callback in &registration.callbacks {
            if callback.registrar != registration.operation
                || !callback.entry_admitted
                || !callback_ids.insert(callback.application_index)
                || previous_callback.is_some_and(|previous| previous >= callback.application_index)
            {
                return malformed("interrupt callback does not belong to its exact installation");
            }
            previous_callback = Some(callback.application_index);
        }
    }
    if !calls.is_empty() {
        return malformed("interrupt installation call has no checked registrar and slot record");
    }
    Ok(())
}

/// Installing handlers changes CPU state, so this operation requires the core
/// MachineControl service. Check the selected declaration and its membership
/// in the ordinary core declaration's published reach. A fixed contribution
/// is separate bookkeeping for installation-bound requirements and is empty
/// for this declaration. This check does not grant native execution authority.
fn validate_control_service(
    module: &TerminalModule,
    selected: semantic_vocabulary::ServiceId,
    published_reach: &[semantic_vocabulary::ServiceId],
) -> Result<(), CodecError> {
    let required = language_core::static_interrupts::StaticInterruptOperation::Install
        .required_service_identity();
    let identity = module
        .services
        .iter()
        .find(|service| service.id == selected)
        .and_then(|service| module.identities.text(service.identity));
    if required.is_none() || identity != required || !published_reach.contains(&selected) {
        return malformed("interrupt installation lost its exact core control service");
    }
    Ok(())
}
