//! Retain the ordinary operands and selected calls of guarded installation.
//!
//! A scope owns one activation. Its native implementation must finish every
//! refusal check before publishing CPU state, then retain these exact loans
//! through the selected nonreturning work call.

use super::super::{NativeRealizationInput, ProviderAdmissionRequest};
use crate::{
    AdmittedFatalInterruptEntry, AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptScope,
    AdmittedInterruptContextAnchor,
};
use language_core::static_interrupts::StaticInterruptOperation;
use terminal_psi::{OperationKind, OperationResult, StructuralAccess};

pub(super) fn prepare(
    input: &NativeRealizationInput<'_>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
    installation: Option<&terminal_installation::AdmittedProviderSelections>,
    entries: &[AdmittedInstalledInterruptEntry],
    helpers: &[crate::AdmittedInstalledInterruptHelper],
    operations: &[(semantic_vocabulary::OperationId, &terminal_psi::Operation)],
) -> Result<Vec<AdmittedInstalledInterruptScope>, String> {
    let mut scopes = Vec::new();
    let module = input.module();
    let mut ordered_entries: Vec<_> = entries.iter().collect();
    ordered_entries.sort_unstable_by_key(|entry| entry.operation);
    let mut helper_roots = helpers
        .iter()
        .map(|helper| helper.installation())
        .collect::<Vec<_>>();
    helper_roots.sort_unstable();
    helper_roots.dedup();

    let calls = super::super::selected_synchronous_call_graph(
        artifact.view(),
        input.target_machine_selections(),
        input
            .selected_program_entry_call()
            .map(|entry| (entry.boundary(), entry.entry())),
        installation,
        request.profile,
    )?;
    // A private callback's provider installation describes its own executed
    // calls. The original program entry is checked with the choices already
    // admitted for that program, retaining its exact selected entry bridge.
    let program_calls = if let Some(context) = request.program_provider_context {
        context.require(
            artifact.view().identity(),
            request.profile,
            request.selected_provider_plans,
        )?;
        Some(super::super::selected_synchronous_call_graph(
            artifact.view(),
            input.target_machine_selections(),
            context.entry_bridge(),
            context.providers(),
            request.profile,
        )?)
    } else {
        None
    };
    let assumed_calls = program_calls.as_ref().unwrap_or(&calls);
    let mut reached = input
        .plan()
        .operations()
        .filter_map(|operation| operation.psi_operation())
        .collect::<Vec<_>>();
    reached.sort_unstable();
    reached.dedup();
    // One graph per distinct assumed entry; registrations reuse its exact
    // selected closure rather than repeatedly walking the program.
    let mut assumed_entries = Vec::new();
    for assumption in request.installation_assumptions {
        assumption.validate_description()?;
        if assumption.psi() != input.plan().psi || assumption.profile() != request.profile {
            return Err("installation assumption names another program or target".into());
        }
        if assumption.entry() != request.bootstrap_entry
            && !request
                .callback_occurrences
                .iter()
                .any(|row| row.callback_thunk_artifact().entry() == assumption.entry())
        {
            return Err(
                "installation assumption names no admitted semantic or callable entry".into(),
            );
        }
        assumed_entries.push(assumption.entry());
    }
    assumed_entries.sort_unstable();
    assumed_entries.dedup();
    let assumed_entries = assumed_entries
        .into_iter()
        .map(|entry| {
            assumed_calls
                .from_root(entry)
                .map(|closure| (entry, closure))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for registration in &module.installed_interrupt_registrations {
        // Installation entries are scoped by the actual executable operations.
        // A private handler re-entry does not execute its original Scope call.
        let first =
            ordered_entries.partition_point(|entry| entry.operation < registration.operation);
        let last =
            ordered_entries.partition_point(|entry| entry.operation <= registration.operation);
        if first == last && helper_roots.binary_search(&registration.operation).is_err() {
            continue;
        }
        let operation = operations
            .binary_search_by_key(&registration.operation, |(id, _)| *id)
            .ok()
            .map(|position| operations[position].1)
            .ok_or("installation has no unique canonical operation")?;
        let OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            ..
        } = &operation.kind
        else {
            return Err("installation changed its operation kind".into());
        };
        let declaration = module
            .boundary_machine(*boundary)
            .ok_or("installation boundary is missing")?;
        let failure = |reason| format!("installation {:?}: {reason}", operation.id);
        // The authenticated core declaration has an ordinary `reaches` row.
        // Its concrete contract is the published ceiling; fixed_service_reach
        // is only the separate basis of an installation-bound requirement.
        // This correspondence does not discharge native authority review.
        if !declaration
            .published_service_ceiling
            .contains(&registration.service)
        {
            return Err(failure(format!(
                "selected service {:?} is outside boundary {:?}'s published reach {:?}",
                registration.service, boundary, declaration.published_service_ceiling
            )));
        }
        if !module
            .services
            .iter()
            .any(|service| service.id == registration.service)
        {
            return Err(failure(format!(
                "selected service {:?} has no declaration",
                registration.service
            )));
        }
        if *boundary != registration.installer {
            return Err(failure(format!(
                "call boundary {boundary:?} differs from retained installer {:?}",
                registration.installer
            )));
        }
        if !arguments.is_empty() {
            return Err(failure(format!(
                "expected no scalar operands, found {arguments:?}"
            )));
        }
        let execution = declaration
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.execution.static_interrupt_operation());
        if execution != Some(StaticInterruptOperation::Install) {
            return Err(failure(format!(
                "boundary has operation {execution:?}, expected Install"
            )));
        }
        if declaration.structural_parameters.len() != 6 {
            return Err(failure(format!(
                "expected six structural formals, found {}",
                declaration.structural_parameters.len()
            )));
        }
        if !matches!(operation.result, OperationResult::Scalar(result) if result.scalar_type == semantic_vocabulary::ScalarType::Boolean)
        {
            return Err(failure(format!(
                "expected Boolean refusal result, found {:?}",
                operation.result
            )));
        }
        let arguments: [terminal_psi::StructuralArgument; 6] = structural_arguments
            .clone()
            .try_into()
            .map_err(|_| "installation must retain all six ordinary operands")?;
        if arguments
            .iter()
            .zip(&declaration.structural_parameters)
            .any(|(actual, formal)| actual.access != formal.access)
            || arguments[0].access != StructuralAccess::MutableBorrow
            || arguments[1].access != StructuralAccess::MutableBorrow
            || arguments[2].access != StructuralAccess::SharedBorrow
            || arguments[5].access != StructuralAccess::SharedBorrow
        {
            return Err(
                "installation changed its exclusive table/GDT or shared TSS/context loans".into(),
            );
        }
        let validator = super::arrival_helpers::selected_body(module, operation.id, 1)?;
        let work = super::arrival_helpers::selected_body(module, operation.id, 2)?;
        let completion = super::arrival_helpers::selected_body(module, operation.id, 3)?;
        let acknowledgement_application = super::arrival_helpers::acknowledgement_entry(
            module,
            registration,
            request.profile,
            request.installed_calling,
            input.target_machine_selections(),
        )?;
        let table_type = declaration.structural_parameters[0].structural_type;
        let context_type = declaration.structural_parameters[5].structural_type;
        for entry in &ordered_entries[first..last] {
            require_table_destination(module, entry, table_type)?;
        }
        require_helper_input(module, validator, table_type, true)?;
        require_helper_input(module, work, context_type, false)?;
        require_retained_work(module, work)?;
        // Boundary verdicts belong to this registration's service declaration.
        let mut configuration = super::configuration::ConfigurationLease::new(registration.service);
        configuration.require(module, &calls.from_root(work)?)?;
        let mut fatal_entries = Vec::new();
        let mut anchor = None;
        let mut gates = Vec::with_capacity(last - first);
        let mut table_bytes = None;
        for entry in &ordered_entries[first..last] {
            let body = entry.entry().machine();
            let closure = calls.from_root(body)?;
            configuration.require(module, &closure)?;
            configuration.require_masked(&closure)?;
            let gate = context_anchor(entry)?;
            if table_bytes.is_some_and(|prior| prior != gate.table_bytes) {
                return Err("installed entries disagree on their table's complete extent".into());
            }
            table_bytes = Some(gate.table_bytes);
            gates.push(gate.gate_byte_offset);
            if entry.placement.application_index == acknowledgement_application {
                if anchor.replace(gate).is_some() {
                    return Err("installation repeats its designated timer entry".into());
                }
                continue;
            }
            require_helper_input(module, body, context_type, false)?;
            if terminal_semantics::control_graph::normal_completion_possible(module, body)
                .map_err(|error| format!("fatal handler has invalid control flow: {error:?}"))?
            {
                return Err(format!(
                    "installed fatal handler {body:?} can return to an uninterpreted hardware frame",
                ));
            }
            match entry.entry().inbound().state.stack {
                calling_conventions::EntryStack::Interrupted
                | calling_conventions::EntryStack::Dedicated { .. } => {}
                _ => {
                    return Err(
                        "fatal handler must name its interrupted or dedicated stack domain".into(),
                    );
                }
            }
            if entry.entry().inbound().call.entry_control
                != calling_conventions::EntryControl::InterruptReturn
            {
                return Err(
                    "fatal handler does not use the admitted interrupt arrival family".into(),
                );
            }
            fatal_entries.push(AdmittedFatalInterruptEntry {
                application: entry.placement.application_index,
                gate_byte_offset: gate.gate_byte_offset,
                machine: body,
            });
        }
        gates.sort_unstable();
        if gates.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("installed callbacks overlap one hardware gate".into());
        }
        // Private-body reentry retains Scope's helper contract but admits only
        // its own callback. The executing installation must own the full anchor.
        if anchor.is_none()
            && input
                .plan()
                .operations()
                .any(|row| row.psi_operation() == Some(operation.id))
        {
            return Err("executing installation has no designated timer gate".into());
        }
        fatal_entries.sort_unstable_by_key(|entry| entry.application);
        let installation_assumptions: Vec<_> = request
            .installation_assumptions
            .iter()
            .filter(|row| row.installation() == operation.id)
            .cloned()
            .collect();
        let mut bootstrap = None;
        let mut delivery = None;
        for assumption in &installation_assumptions {
            let occupied = match assumption {
                installation_evidence::InstallationEnvironmentAssumption::Bootstrap(row) => {
                    bootstrap.replace(row).is_some()
                }
                installation_evidence::InstallationEnvironmentAssumption::InterruptDelivery(
                    row,
                ) => delivery.replace(row).is_some(),
            };
            if occupied {
                return Err("installation repeats one kind of platform assumption".into());
            }
            let index = assumed_entries
                .binary_search_by_key(&assumption.entry(), |row| row.0)
                .map_err(|_| "installation assumption lost its exact entry closure")?;
            if !assumed_entries[index]
                .1
                .machines
                .iter()
                .any(|machine| machine.operations().any(|row| row.id == operation.id))
            {
                return Err(
                    "installation assumption entry does not reach this installation".into(),
                );
            }
        }
        if matches!((bootstrap, delivery), (Some(left), Some(right)) if left.entry != right.entry) {
            return Err("installation conditions name different physical entries".into());
        }
        if reached.binary_search(&operation.id).is_ok() {
            if bootstrap.is_none() {
                return Err("installation requires an explicit bootstrap environment assumption through publication".into());
            }
            if !fatal_entries.is_empty() && delivery.is_none() {
                return Err("fatal installation requires an explicit interrupt delivery assumption for its installed lifetime".into());
            }
        }
        scopes.push(AdmittedInstalledInterruptScope {
            service: registration.service,
            operation: operation.id,
            boundary: *boundary,
            arguments,
            table_type,
            context_type,
            validator,
            work,
            completion,
            acknowledgement_application,
            fatal_entries,
            anchor,
            installation_assumptions,
        });
    }
    Ok(scopes)
}

fn require_helper_input(
    module: &terminal_psi::TerminalModule,
    helper: semantic_vocabulary::MachineId,
    referent: semantic_vocabulary::StructuralTypeId,
    returns_boolean: bool,
) -> Result<(), String> {
    let body = module
        .machine(helper)
        .ok_or("installation helper body is missing")?;
    let [parameter] = body.structural_parameters.as_slice() else {
        return Err("installation helper must receive one exact retained borrow".into());
    };
    if !body.parameters.is_empty()
        || parameter.structural_type != referent
        || parameter.access != StructuralAccess::SharedBorrow
    {
        return Err("installation helper changed its retained table/context input".into());
    }
    let valid_result = if returns_boolean {
        matches!(body.result, terminal_psi::TerminalMachineResult::Scalar(result)
            if result.scalar_type == semantic_vocabulary::ScalarType::Boolean)
    } else {
        body.result == terminal_psi::TerminalMachineResult::Unit
    };
    if !valid_result {
        return Err("installation helper changed its checked result".into());
    }
    Ok(())
}

/// Publication keeps this activation and its loans live. A source promise alone
/// cannot justify freeing that storage on a later normal return. Reconstruct
/// the selected body's actual control before retaining its activation.
fn require_retained_work(
    module: &terminal_psi::TerminalModule,
    work: semantic_vocabulary::MachineId,
) -> Result<(), String> {
    if terminal_semantics::control_graph::normal_completion_possible(module, work)
        .map_err(|error| format!("installation work has invalid control flow: {error:?}"))?
    {
        return Err(
            "installation work can return while its table and context remain published".into(),
        );
    }
    Ok(())
}

/// The sealed registrar names a formal, not a second runtime table. Bind that
/// exact formal's retained backing type to Scope's actual exclusive table loan
/// before any native address store can use its placement recipe.
fn require_table_destination(
    module: &terminal_psi::TerminalModule,
    entry: &AdmittedInstalledInterruptEntry,
    table: semantic_vocabulary::StructuralTypeId,
) -> Result<(), String> {
    let materialization = entry
        .placement
        .private_materialization
        .as_ref()
        .ok_or("installed entry has no private table destination")?;
    let calling_conventions::NativePlace::Field { parameter, .. } = materialization.destination
    else {
        return Err("installed entry requires a field in the retained table".into());
    };
    let signature = module
        .calling_abi_signature(materialization.registrar_application.abi.id())
        .ok_or("installed table destination lost its exact registrar ABI")?;
    if signature.identity(&module.identities)? != materialization.registrar_application.abi {
        return Err("installed table destination changed its full registrar ABI".into());
    }
    let storage = signature.storage.as_ref().map_err(Clone::clone)?;
    let mut matching = storage
        .telescope
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.identity == parameter);
    let (Some((ordinal, _)), None) = (matching.next(), matching.next()) else {
        return Err("installed table destination has no unique registrar formal".into());
    };
    let formal = storage
        .parameters
        .get(ordinal)
        .ok_or("installed table destination has no retained formal storage")?;
    if formal.access != StructuralAccess::MutableBorrow || formal.callback_backing != Some(table) {
        return Err(
            "installed registrar destination is not Scope's exact exclusive table type".into(),
        );
    }
    Ok(())
}

/// The independently admitted Calling application owns the complete destination
/// layout. Recover its gate before emission; never interpret a stack class as IST.
fn context_anchor(
    entry: &AdmittedInstalledInterruptEntry,
) -> Result<AdmittedInterruptContextAnchor, String> {
    if entry.entry().target().architecture != target::Architecture::X86_64 {
        return Err("installed interrupt gate geometry requires x86-64".into());
    }
    let materialization = entry
        .placement
        .private_materialization
        .as_ref()
        .ok_or("installed entry has no retained table materialization")?;
    let signature = &materialization.registrar_application.signature;
    let layouts = signature
        .callback_layouts
        .span(signature.callback_layout_rows)
        .ok_or("installed entry has a stale callback layout roster")?;
    let mut matching = layouts
        .iter()
        .filter(|layout| layout.destination == materialization.destination);
    let (Some(layout), None) = (matching.next(), matching.next()) else {
        return Err("installed entry has no unique destination layout".into());
    };
    let table_bytes =
        u32::try_from(layout.root.size).map_err(|_| "installed table exceeds native storage")?;
    let placements = materialization
        .destination_application
        .placements()
        .ok_or("installed entry does not occupy a table field")?;
    let gate = calling_conventions::interrupt_gate_address(placements.as_slice(), table_bytes)?;
    Ok(AdmittedInterruptContextAnchor {
        application: entry.placement.application_index,
        function: entry.callback_function,
        destination: materialization.destination.clone(),
        gate_byte_offset: gate.gate_byte_offset,
        table_bytes,
    })
}
