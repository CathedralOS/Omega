/*
Service reach, judged on the lowered module.

`wiki/spec/language/effects.md`, "Declarations": "A present clause is an
upper bound ... The complete derived reach must fit its normalized ceiling.
`reaches A + B` permits a body reaching only `A`; `reaches none` rejects any
nonempty derived service row." and "A derived reach that exceeds its ceiling
rejects at the call that contributes the excess. The diagnostic names that
call, the identities its requirement declares, and the ceiling."

Every machine of the module publishes a row of services
(`TerminalMachine::published_service_ceiling`). A call contributes the row
its callee publishes. The rule read back from the module is, for each call:

- in a machine whose header wrote a ceiling, what the call contributes lies
  inside that ceiling; one that does not is the program's error, reported at
  the call;
- in every machine, what the call contributes lies inside the row the caller
  itself publishes, so no caller hides a callee's reach from its own callers.

The first is the author's promise and the second is the inference. Only the
ceiling comes from elaboration (`Elaboration::reach`), because the module
cannot tell `reaches none` from an omitted clause; the rows and the calls are
read from the module, not from the pass that wrote them. A row that is
unordered, names an undeclared service, lost a parent, or breaks the second
rule where no ceiling was written is a fault of the tables, never a verdict
about the program.

This is a judgment over finite sets with no certificate, of the same
standing as a loan conflict.
*/

use diagnostics::Diagnostic;
use semantic_vocabulary::{InvocationSite, MachineId, ServiceId};
use structure_elaboration::{Elaboration, SourceOrigin};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{TerminalMachine, TerminalModule};

use crate::refusal;

/// One call that contributes services its caller's written ceiling leaves
/// out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceededReach {
    pub machine: MachineId,
    /// The call: an operation, or the edge of a call that does not return.
    pub at: Construct,
    pub callee: Callee,
    /// The services the call contributes beyond the ceiling, ascending.
    pub undeclared: Vec<ServiceId>,
    /// Where the call arises in the source.
    pub site: SourceOrigin,
    /// What exceeds what, in the current checker's words for the machine and
    /// then the call. Diagnostic text only.
    pub statement: String,
}

impl ExceededReach {
    pub fn message(&self) -> String {
        self.statement.clone()
    }

    /// The `Reject` diagnostic, at the call.
    pub fn reject_diagnostic(&self) -> Diagnostic {
        let diagnostic = Diagnostic::error(self.message());
        match self.site.span() {
            Some(span) => diagnostic.with_source_span(span),
            None => diagnostic,
        }
    }
}

fn fault(machine: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}

/// A row is ascending, names declared services only and holds every parent
/// of each service it names.
pub(crate) fn well_formed(
    module: &TerminalModule,
    machine: MachineId,
    row: &[ServiceId],
) -> Checked<()> {
    if row.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(fault(machine, "a service row is not strictly ordered"));
    }
    for service in row {
        let declared = module
            .services
            .iter()
            .find(|declared| declared.id == *service)
            .ok_or_else(|| fault(machine, "a service row names an undeclared service"))?;
        if declared
            .parents
            .iter()
            .any(|parent| row.binary_search(parent).is_err())
        {
            return Err(fault(machine, "a service row lost a boundary parent"));
        }
    }
    Ok(())
}

fn beyond(row: &[ServiceId], bound: &[ServiceId]) -> Vec<ServiceId> {
    row.iter()
        .copied()
        .filter(|service| bound.binary_search(service).is_err())
        .collect()
}

fn machine_at(module: &TerminalModule, id: MachineId) -> Option<&TerminalMachine> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| module.machines.get(position))
        .filter(|machine| machine.id == id)
}

fn row_text(elaboration: &Elaboration, row: &[ServiceId]) -> String {
    if row.is_empty() {
        return String::from("<none>");
    }
    row.iter()
        .map(|service| {
            usize::try_from(service.get() - 1)
                .ok()
                .and_then(|position| elaboration.reach.names.get(position))
                .map_or("<unnamed service>", String::as_str)
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn machine_name(elaboration: &Elaboration, machine: MachineId) -> String {
    elaboration
        .names
        .get(machine)
        .map(|names| names.machine.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("machine {}", machine.get()))
}

/// Every call of `elaboration`'s module that exceeds its caller's written
/// ceiling, in machine and block order. `Err` when the module's rows are not
/// what the rule above requires of them, or when a call's reach is not
/// served.
fn validate_envelope(
    module: &TerminalModule,
    machine: MachineId,
    envelope: &terminal_psi::TerminalCallableEnvelope,
) -> Checked<()> {
    well_formed(module, machine, &envelope.service_reach)?;
    if envelope
        .invocations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(fault(
            machine,
            "callable invocation envelope is not strictly ordered",
        ));
    }
    for target in &envelope.invocations {
        if let terminal_psi::TerminalCallableInvocation::Service(service) = target
            && module
                .services
                .iter()
                .all(|declaration| declaration.id != *service)
        {
            return Err(fault(
                machine,
                "callable invocation envelope names an absent service",
            ));
        }
    }
    Ok(())
}

fn occurrence_sites(machine: &TerminalMachine) -> Checked<Vec<InvocationSite>> {
    let mut sites = machine
        .module_invocations()
        .map(|call| call.site)
        .chain(machine.callable_invocations().map(|call| call.site))
        .chain(machine.boundary_invocations().map(|call| call.site))
        .collect::<Vec<_>>();
    let position = |site: &InvocationSite| {
        machine.blocks.iter().enumerate().find_map(|(block_position, block)| {
        match site {
            InvocationSite::Operation(operation) => block.operations.iter().position(|candidate| candidate.id == *operation).map(|index| (block_position, index)),
            InvocationSite::InvokeNoNormal(edge) => matches!(block.terminator, terminal_psi::Terminator::InvokeNoNormal { edge: actual, .. } if actual == *edge).then_some((block_position, block.operations.len())),
            InvocationSite::ExternalCompletion(edge) => matches!(block.terminator, terminal_psi::Terminator::ExternalCompletion { edge: actual, .. } if actual == *edge).then_some((block_position, block.operations.len())),
        }
    })
    };
    if sites.iter().any(|site| position(site).is_none()) {
        return Err(fault(
            machine.id,
            "a service call has no exact lowered occurrence",
        ));
    }
    sites.sort_by_key(position);
    if sites.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(fault(
            machine.id,
            "a service invocation has duplicate lowered views",
        ));
    }
    Ok(sites)
}

fn contribution<'a>(
    elaboration: &'a Elaboration,
    caller: &'a TerminalMachine,
    site: InvocationSite,
) -> Checked<(Callee, &'a [ServiceId])> {
    let module = &elaboration.module;
    let tables = terminal_check_tables::row(
        &elaboration.tables.machines,
        caller.id,
        caller.id,
        Construct::Machine,
    )?;
    let mut rows = tables
        .invocations
        .iter()
        .filter(|(_, row)| row.site == site);
    let (id, row) = rows
        .next()
        .ok_or_else(|| fault(caller.id, "service invocation has no checking occurrence"))?;
    if rows.next().is_some() {
        return Err(fault(
            caller.id,
            "service invocation has ambiguous checking occurrences",
        ));
    }
    let call = crate::call_contracts::invocation(module, caller, tables, id, row)?;
    let contributed = match call.inputs.callee {
        Callee::Machine(callee) => {
            &machine_at(module, callee)
                .ok_or_else(|| fault(caller.id, "service call names a machine outside the module"))?
                .published_service_ceiling
        }
        Callee::Selected { .. } => {
            let contract = call
                .inputs
                .contract
                .ok_or_else(|| fault(caller.id, "service invocation lost its callable contract"))?;
            &module
                .callables
                .contract(contract)
                .ok_or_else(|| {
                    fault(
                        caller.id,
                        "service invocation names an absent callable contract",
                    )
                })?
                .envelope
                .service_reach
        }
        Callee::Boundary { boundary } => {
            let declaration = module.boundary_machine(boundary).ok_or_else(|| {
                fault(
                    caller.id,
                    "service invocation names an absent boundary declaration",
                )
            })?;
            if declaration.role != terminal_psi::BoundaryDeclarationRole::Callable {
                return Err(fault(
                    caller.id,
                    "contract evidence cannot authorize a boundary invocation",
                ));
            }
            let occurrence = caller
                .boundary_invocations()
                .find(|call| call.site == site)
                .ok_or_else(|| {
                    fault(
                        caller.id,
                        "boundary service call has no exact lowered occurrence",
                    )
                })?;
            let completion_matches = matches!(
                (declaration.completion, occurrence.completion),
                (
                    semantic_vocabulary::BoundaryCompletion::NormalReturn,
                    terminal_psi::control_flow::BoundaryInvocationCompletion::Normal { .. }
                )
            ) || matches!((declaration.completion, occurrence.completion),
                    (semantic_vocabulary::BoundaryCompletion::External(expected), terminal_psi::control_flow::BoundaryInvocationCompletion::External(actual)) if expected == actual);
            if !completion_matches {
                return Err(fault(
                    caller.id,
                    "boundary invocation changes its declared completion",
                ));
            }
            if declaration.kind == semantic_vocabulary::BoundaryRequirementKind::Trait
                && declaration.owning_service.is_none()
            {
                return Err(fault(
                    caller.id,
                    "boundary invocation lost its owning service",
                ));
            }
            &declaration.published_service_ceiling
        }
    };
    well_formed(module, caller.id, contributed)?;
    Ok((call.inputs.callee, contributed))
}

fn callee_name(elaboration: &Elaboration, callee: Callee) -> String {
    match callee {
        Callee::Machine(machine) => machine_name(elaboration, machine),
        Callee::Selected { callable } => format!("stored callable value {}", callable.get()),
        Callee::Boundary { boundary } => elaboration
            .module
            .boundary_machine(boundary)
            .and_then(|declaration| elaboration.module.identities.text(declaration.identity))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("boundary {}", boundary.get())),
    }
}

pub(crate) fn judge(elaboration: &Elaboration) -> Checked<Vec<ExceededReach>> {
    let module = &elaboration.module;
    let first = MachineId::new(1).expect("1 is nonzero");
    for (position, service) in module.services.iter().enumerate() {
        if (position > 0 && module.services[position - 1].id >= service.id)
            || service.parents.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(fault(first, "service namespace is not canonical"));
        }
        let identity = module
            .identities
            .text(service.identity)
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| fault(first, "service has no retained declaration identity"))?;
        if position > 0
            && module
                .identities
                .text(module.services[position - 1].identity)
                .is_none_or(|previous| previous >= identity)
        {
            return Err(fault(
                first,
                "service declaration identities are not strictly ordered",
            ));
        }
        if service.parents.iter().any(|parent| {
            module
                .services
                .binary_search_by_key(parent, |declaration| declaration.id)
                .is_err()
        }) {
            return Err(fault(
                first,
                "service inheritance names an absent declaration",
            ));
        }
    }
    let mut colors = vec![0u8; module.services.len()];
    for root in 0..module.services.len() {
        let mut pending = vec![(root, false)];
        while let Some((position, exiting)) = pending.pop() {
            if exiting {
                colors[position] = 2;
                continue;
            }
            match colors[position] {
                1 => return Err(fault(first, "service inheritance has a cycle")),
                2 => continue,
                _ => {}
            }
            colors[position] = 1;
            pending.push((position, true));
            for parent in &module.services[position].parents {
                let parent = module
                    .services
                    .binary_search_by_key(parent, |declaration| declaration.id)
                    .map_err(|_| fault(first, "service parent lost its exact declaration"))?;
                pending.push((parent, false));
            }
        }
    }
    for contract in &module.callables.contracts {
        validate_envelope(module, first, &contract.envelope)?;
    }
    for named in module.callables.machine_envelopes.iter().flatten() {
        validate_envelope(module, named.machine, &named.envelope)?;
    }
    for boundary in &module.boundary_machines {
        well_formed(module, first, &boundary.fixed_service_reach)?;
        well_formed(module, first, &boundary.published_service_ceiling)?;
        if !beyond(
            &boundary.fixed_service_reach,
            &boundary.published_service_ceiling,
        )
        .is_empty()
        {
            return Err(fault(
                first,
                "boundary ceiling omits its fixed service demand",
            ));
        }
    }
    let mut exceeded = Vec::new();
    for machine in &module.machines {
        let id = machine.id;
        let published = machine.published_service_ceiling.as_slice();
        well_formed(module, id, published)?;
        well_formed(module, id, &machine.declared_service_reach)?;
        if module
            .callables
            .machine_envelope(id)
            .is_some_and(|envelope| envelope.service_reach != published)
        {
            return Err(fault(
                id,
                "named callable envelope differs from its published service row",
            ));
        }
        let ceiling = elaboration
            .reach
            .ceilings
            .get(id)
            .and_then(|ceiling| ceiling.as_deref());
        match ceiling {
            Some(ceiling) if machine.declared_service_reach != ceiling => {
                return Err(fault(id, "the declared reach is not the written ceiling"));
            }
            None if !machine.declared_service_reach.is_empty() => {
                return Err(fault(
                    id,
                    "a reach is declared where no ceiling was written",
                ));
            }
            _ => {}
        }
        let sites = occurrence_sites(machine)?;
        let tables =
            terminal_check_tables::row(&elaboration.tables.machines, id, id, Construct::Machine)?;
        if tables
            .invocations
            .values()
            .any(|row| !sites.contains(&row.site))
        {
            return Err(fault(
                id,
                "a service checking row restates no lowered invocation",
            ));
        }
        for occurrence in sites {
            let (callee, contributed) = contribution(elaboration, machine, occurrence)?;
            let undeclared = beyond(contributed, ceiling.unwrap_or(published));
            if undeclared.is_empty() {
                if !beyond(contributed, published).is_empty() {
                    return Err(fault(id, "the published reach omits a callee's reach"));
                }
                continue;
            }
            let Some(ceiling) = ceiling else {
                return Err(fault(id, "the published reach omits a callee's reach"));
            };
            let (at, site) = match occurrence {
                InvocationSite::Operation(operation) => (
                    Construct::Operation(operation),
                    elaboration
                        .sources
                        .get(id)
                        .and_then(|sources| sources.operations.get(operation)),
                ),
                InvocationSite::InvokeNoNormal(edge) | InvocationSite::ExternalCompletion(edge) => {
                    (
                        Construct::Edge(edge),
                        elaboration
                            .sources
                            .get(id)
                            .and_then(|sources| sources.edges.get(edge)),
                    )
                }
            };
            let site = *site.ok_or_else(|| fault(id, "a call has no source correspondence"))?;
            let statement = format!(
                "machine `{}` publishes service reach `{}` but its checked body reaches undeclared service{} `{}`: the call of `{}` contributes `{}`",
                machine_name(elaboration, id),
                row_text(elaboration, ceiling),
                if undeclared.len() == 1 { "" } else { "s" },
                row_text(elaboration, &undeclared),
                callee_name(elaboration, callee),
                row_text(elaboration, contributed)
            );
            exceeded.push(ExceededReach {
                machine: id,
                at,
                callee,
                undeclared,
                site,
                statement,
            });
        }
        if ceiling.is_some_and(|ceiling| !beyond(published, ceiling).is_empty()) {
            return Err(fault(id, "the published reach exceeds the written ceiling"));
        }
    }
    if let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry
        && machine_at(module, entry).is_none_or(|machine| {
            machine.published_service_ceiling != module.root_service_reach.concrete
        })
    {
        return Err(fault(
            entry,
            "the root reach is not the entry's published reach",
        ));
    }
    Ok(exceeded)
}
