// Guarded interrupt installation scope preparation and CPU configuration lease
// enforcement.
//
// A scope owns one activation: its native implementation must finish every
// refusal check before publishing CPU state, then retain the six exclusive and
// shared loans (`table`, `GDT`, `TSS`, and `context`) through the selected
// nonreturning `work` call.
//
// Borrowed table and TSS storage cannot by itself prevent a separate
// instruction from replacing a CPU register, so `ConfigurationLease` inspects
// the complete selected execution closure of `work` and every handler entry to
// reject instructions that replace GDTR, TR, segment descriptors, or system/MSR
// registers, and to require interrupt-gate handlers to keep maskable arrivals
// disabled.

use super::super::{AdmissionSource, ProviderAdmissionRequest};
use crate::{
    AdmittedFatalInterruptEntry, AdmittedInstalledInterruptEntry, AdmittedInstalledInterruptScope,
    AdmittedInterruptContextAnchor,
};
use arena::IdSet;
use language_core::inline_assembly::AsmGlobalDescriptorTableOperation;
use language_core::static_interrupts::StaticInterruptOperation;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};
use terminal_psi::{
    OperationKind, OperationResult, RegisterOperation, StructuralAccess, TerminalModule,
};

pub(super) fn prepare(
    input: &AdmissionSource<'_>,
    analyses: Option<&terminal_semantics::ModuleAnalyses<'_>>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    request: &ProviderAdmissionRequest<'_, '_>,
    installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
    entries: &[AdmittedInstalledInterruptEntry],
    helpers: &[crate::AdmittedInstalledInterruptHelper],
    operations: &[(super::OperationSite, &terminal_psi::Operation)],
) -> Result<Vec<AdmittedInstalledInterruptScope>, String> {
    let mut scopes = Vec::new();
    let module = input.module();
    let mut ordered_entries: Vec<_> = entries.iter().collect();
    ordered_entries.sort_unstable_by_key(|entry| (entry.machine, entry.operation));
    let mut helper_roots = helpers
        .iter()
        .map(|helper| (helper.installation_machine, helper.installation()))
        .collect::<Vec<_>>();
    helper_roots.sort_unstable();
    helper_roots.dedup();

    let calls = super::super::selected_synchronous_call_graph(
        artifact.view(),
        analyses,
        input.target_context(),
        input
            .selected_program_entry_call()
            .map(|entry| (entry.boundary(), entry.entry())),
        installation,
        request.profile,
    )?;
    let program_calls = if let Some(context) = request.program_provider_context {
        context.require(
            artifact.view().identity(),
            request.profile,
            request.selected_provider_plans,
        )?;
        Some(super::super::selected_synchronous_call_graph(
            artifact.view(),
            analyses,
            input.target_context(),
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
        .functions
        .iter()
        .flat_map(|function| {
            function.operations().filter_map(move |operation| {
                operation
                    .psi_operation()
                    .map(|operation| (function.machine, operation))
            })
        })
        .collect::<Vec<_>>();
    reached.sort_unstable();
    reached.dedup();
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
    let assumed_installations = assumed_installation_sites(module, &assumed_entries);
    for registration in &module.interfaces.installed_interrupt_registrations {
        let installation = (registration.machine, registration.operation);
        let first = ordered_entries
            .partition_point(|entry| (entry.machine, entry.operation) < installation);
        let last = ordered_entries
            .partition_point(|entry| (entry.machine, entry.operation) <= installation);
        if first == last && helper_roots.binary_search(&installation).is_err() {
            continue;
        }
        let operation = operations
            .binary_search_by_key(&installation, |(id, _)| *id)
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
            .interfaces.services
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
        let validator =
            super::arrival_helpers::selected_body(module, registration.machine, operation.id, 1)?;
        let work =
            super::arrival_helpers::selected_body(module, registration.machine, operation.id, 2)?;
        let completion =
            super::arrival_helpers::selected_body(module, registration.machine, operation.id, 3)?;
        let acknowledgement_application = super::arrival_helpers::acknowledgement_entry(
            module,
            registration,
            request.profile,
            request.installed_calling,
        )?;
        let table_type = declaration.structural_parameters[0].structural_type;
        let context_type = declaration.structural_parameters[5].structural_type;
        for entry in &ordered_entries[first..last] {
            require_table_destination(module, entry, table_type)?;
        }
        require_helper_input(module, validator, table_type, true)?;
        require_helper_input(module, work, context_type, false)?;
        require_retained_work(module, work)?;
        let mut configuration = ConfigurationLease::new(registration.service);
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
        if anchor.is_none() && reached.binary_search(&installation).is_ok() {
            return Err("executing installation has no designated timer gate".into());
        }
        fatal_entries.sort_unstable_by_key(|entry| entry.application);
        let mut installation_assumptions = Vec::new();
        for assumption in request
            .installation_assumptions
            .iter()
            .filter(|row| row.installation() == operation.id)
        {
            let Some(Some(owner)) =
                assumed_installations.get(&(assumption.entry(), assumption.installation()))
            else {
                return Err(
                    "installation assumption entry has no unique matching installation".into(),
                );
            };
            if *owner == installation {
                installation_assumptions.push(assumption.clone());
            }
        }
        let mut bootstrap = None;
        let mut delivery = None;
        for assumption in &installation_assumptions {
            let occupied = match assumption {
                installation_conditions::InstallationEnvironmentAssumption::Bootstrap(row) => {
                    bootstrap.replace(row).is_some()
                }
                installation_conditions::InstallationEnvironmentAssumption::InterruptDelivery(
                    row,
                ) => delivery.replace(row).is_some(),
            };
            if occupied {
                return Err("installation repeats one kind of platform assumption".into());
            }
        }
        if matches!((bootstrap, delivery), (Some(left), Some(right)) if left.entry != right.entry) {
            return Err("installation conditions name different physical entries".into());
        }
        if reached.binary_search(&installation).is_ok() {
            if bootstrap.is_none() {
                return Err("installation requires an explicit bootstrap environment assumption through publication".into());
            }
            if !fatal_entries.is_empty() && delivery.is_none() {
                return Err("fatal installation requires an explicit interrupt delivery assumption for its installed lifetime".into());
            }
        }
        scopes.push(AdmittedInstalledInterruptScope {
            facts: std::sync::Arc::new(bound_installations::BoundInterruptInstallation {
                machine: registration.machine,
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
                context_anchor: anchor,
                installation_assumptions,
            }),
        });
    }
    Ok(scopes)
}

// The public assumption names an entry and a machine-local operation.
// Resolve that pair only when the selected entry closure contains one exact
// checked installation. Two owners with the same local number are ambiguous.
fn assumed_installation_sites(
    module: &TerminalModule,
    assumed_entries: &[(MachineId, terminal_semantics::synchronous_calls::SynchronousCallClosure<'_>)],
) -> std::collections::BTreeMap<(MachineId, OperationId), Option<super::OperationSite>> {
    let registration_sites: std::collections::BTreeMap<_, _> = module
        .interfaces.installed_interrupt_registrations
        .iter()
        .map(|registration| {
            (
                (registration.machine, registration.operation),
                registration.installer,
            )
        })
        .collect();
    let mut assumed_installations: std::collections::BTreeMap<
        (MachineId, OperationId),
        Option<super::OperationSite>,
    > = std::collections::BTreeMap::new();
    for (entry, closure) in assumed_entries {
        for machine in &closure.machines {
            for operation in machine.operations() {
                let site = (machine.id, operation.id);
                let Some(installer) = registration_sites.get(&site) else {
                    continue;
                };
                if !matches!(&operation.kind, OperationKind::BoundaryCall { boundary, .. }
                    if boundary == installer)
                {
                    continue;
                }
                assumed_installations
                    .entry((*entry, operation.id))
                    .and_modify(|current| {
                        if *current != Some(site) {
                            *current = None;
                        }
                    })
                    .or_insert(Some(site));
            }
        }
    }
    assumed_installations
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

struct ConfigurationLease {
    machine_control: ServiceId,
    bodies: IdSet<MachineId>,
    boundaries: IdSet<BoundaryMachineId>,
    masked_bodies: IdSet<MachineId>,
}

impl ConfigurationLease {
    fn new(machine_control: ServiceId) -> Self {
        Self {
            machine_control,
            bodies: IdSet::default(),
            boundaries: IdSet::default(),
            masked_bodies: IdSet::default(),
        }
    }

    fn require(
        &mut self,
        module: &TerminalModule,
        closure: &terminal_semantics::synchronous_calls::SynchronousCallClosure<'_>,
    ) -> Result<(), String> {
        for body in &closure.machines {
            if !self.bodies.insert(body.id) {
                continue;
            }
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                let replaces_configuration = match &operation.kind {
                    OperationKind::ActivateTaskRegister { .. }
                    | OperationKind::ReloadSegmentDescriptors { .. } => true,
                    OperationKind::GlobalDescriptorTable { operation, .. } => {
                        *operation == AsmGlobalDescriptorTableOperation::Load
                    }
                    OperationKind::RegisterAccess { event, .. } => {
                        changes_configuration(event.operation())
                    }
                    _ => false,
                };
                if replaces_configuration {
                    return Err(format!(
                        "installed context requires stable CPU configuration; machine {:?} operation {:?} can replace it",
                        body.id, operation.id,
                    ));
                }
            }
        }
        for &boundary in &closure.boundaries {
            if !self.boundaries.insert(boundary) {
                continue;
            }
            let declaration = module
                .boundary_machine(boundary)
                .ok_or("installed execution reaches a missing boundary")?;
            let intrinsic = declaration
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.execution.static_interrupt_operation());
            match intrinsic {
                Some(StaticInterruptOperation::Install) => {
                    return Err("installed execution can publish another table before retiring the live root".into());
                }
                Some(
                    StaticInterruptOperation::Context
                    | StaticInterruptOperation::BeginCompletion
                    | StaticInterruptOperation::FinishCompletion,
                ) => {}
                None if declaration
                    .published_service_ceiling
                    .contains(&self.machine_control) =>
                {
                    return Err(format!(
                        "installed context requires a checked configuration-preserving body for machine-control boundary {boundary:?}",
                    ));
                }
                None => {}
            }
        }
        Ok(())
    }

    fn require_masked(
        &mut self,
        closure: &terminal_semantics::synchronous_calls::SynchronousCallClosure<'_>,
    ) -> Result<(), String> {
        for body in &closure.machines {
            if !self.masked_bodies.insert(body.id) {
                continue;
            }
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                if matches!(&operation.kind,
                    OperationKind::RegisterAccess { event, .. }
                    if event.operation() == RegisterOperation::InterruptControl(
                        language_core::inline_assembly::AsmInterruptControlKind::Enable))
                {
                    return Err(format!(
                        "masked installed handler machine {:?} operation {:?} enables nested maskable arrivals",
                        body.id, operation.id,
                    ));
                }
            }
        }
        Ok(())
    }
}

fn changes_configuration(operation: RegisterOperation) -> bool {
    matches!(
        operation,
        RegisterOperation::GlobalDescriptorTable(AsmGlobalDescriptorTableOperation::Load)
            | RegisterOperation::TaskRegisterActivate
            | RegisterOperation::SegmentReload(_)
            | RegisterOperation::SystemWrite(_)
            | RegisterOperation::ModelSpecificWrite
    )
}

#[cfg(test)]
mod assumption_owner_tests {
    use super::assumed_installation_sites;
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ServiceId,
    };
    use terminal_semantics::synchronous_calls::SynchronousCallClosure;
    use terminal_psi::{
        Block, MachineContract, NominalDeclarationIdentity, Operation, OperationKind,
        OperationResult, RegisterTargetApplicability, TerminalIdentityIndex,
        TerminalInstalledInterruptRegistration, TerminalMachine, TerminalModule, Terminator,
    };

    fn machine(identity: u64, boundary: BoundaryMachineId) -> TerminalMachine {
        let block_id = BlockId::new(1).unwrap();
        let mut block = Block::plain(
            block_id,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        );
        block.operations.push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary,
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        ));
        TerminalMachine {
            id: MachineId::new(identity).unwrap(),
            assembly_target_requirement: RegisterTargetApplicability::Any,
            attachment: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id,
            blocks: vec![block],
            contract: MachineContract::empty(ContractId::new(identity).unwrap()),
        }
    }

    // Only exact operation resolution is under test. Portable record admission
    // and selected call-graph construction have their own validation boundaries.
    fn registration(
        machine: &TerminalMachine,
        installer: BoundaryMachineId,
    ) -> TerminalInstalledInterruptRegistration {
        let nominal = NominalDeclarationIdentity::new(TerminalIdentityIndex(1));
        TerminalInstalledInterruptRegistration {
            machine: machine.id,
            operation: OperationId::new(1).unwrap(),
            installer,
            service: ServiceId::new(1).unwrap(),
            registrar_boundary: installer,
            registrar: nominal.clone(),
            registrar_owner: nominal.clone(),
            registrar_application: String::new(),
            registrar_declaring_application: String::new(),
            registrar_arguments: Vec::new(),
            registrar_lifetimes: Vec::new(),
            registrar_requirement: String::new(),
            slot_declaration: nominal,
            slot_commitment: [0; 32],
            slot_encoding: Vec::new(),
            callbacks: Vec::new(),
        }
    }

    fn closure(machines: Vec<&TerminalMachine>) -> SynchronousCallClosure<'_> {
        SynchronousCallClosure {
            machines,
            boundaries: Default::default(),
        }
    }

    #[test]
    fn assumption_entries_distinguish_installation_owners_and_refuse_ambiguity() {
        let first_boundary = BoundaryMachineId::new(1).unwrap();
        let second_boundary = BoundaryMachineId::new(2).unwrap();
        let first = machine(1, first_boundary);
        let second = machine(2, second_boundary);
        let mut module = TerminalModule::declarations();
        module.interfaces.installed_interrupt_registrations = vec![
            registration(&first, first_boundary),
            registration(&second, second_boundary),
        ];
        let first_entry = MachineId::new(10).unwrap();
        let second_entry = MachineId::new(11).unwrap();
        let ambiguous_entry = MachineId::new(12).unwrap();
        let operation = OperationId::new(1).unwrap();
        let entries = [
            (first_entry, closure(vec![&first])),
            (second_entry, closure(vec![&second])),
            // A later occurrence of the first site cannot erase the ambiguity.
            (ambiguous_entry, closure(vec![&first, &second, &first])),
        ];
        let sites = assumed_installation_sites(&module, &entries);
        assert_eq!(
            sites,
            std::collections::BTreeMap::from([
                ((first_entry, operation), Some((first.id, operation))),
                ((second_entry, operation), Some((second.id, operation))),
                ((ambiguous_entry, operation), None),
            ]),
            "an assumption's entry selects an exact owner rather than sharing its local operation number",
        );
    }

    #[test]
    fn unregistered_or_wrong_boundary_sites_cannot_supply_assumption_authority() {
        let installer = BoundaryMachineId::new(1).unwrap();
        let another_installer = BoundaryMachineId::new(2).unwrap();
        let checked = machine(1, installer);
        let unregistered = machine(2, installer);
        let wrong_boundary = machine(3, installer);
        let mut module = TerminalModule::declarations();
        module.interfaces.installed_interrupt_registrations = vec![
            registration(&checked, installer),
            registration(&wrong_boundary, another_installer),
        ];
        let no_installation = MachineId::new(10).unwrap();
        let one_installation = MachineId::new(11).unwrap();
        let entries = [
            (
                no_installation,
                closure(vec![&unregistered, &wrong_boundary]),
            ),
            (
                one_installation,
                closure(vec![&checked, &unregistered, &wrong_boundary]),
            ),
        ];
        assert_eq!(
            assumed_installation_sites(&module, &entries),
            std::collections::BTreeMap::from([(
                (one_installation, OperationId::new(1).unwrap()),
                Some((checked.id, OperationId::new(1).unwrap())),
            )]),
            "an unrelated same-numbered call neither supplies an installation nor makes a checked site ambiguous",
        );
    }
}
