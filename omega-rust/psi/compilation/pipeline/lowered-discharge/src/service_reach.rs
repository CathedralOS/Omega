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
    pub callee: MachineId,
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
        match self.site {
            SourceOrigin::Authored(span) => diagnostic.with_source_span(span),
            SourceOrigin::Unpositioned(_) => diagnostic,
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
fn well_formed(module: &TerminalModule, machine: MachineId, row: &[ServiceId]) -> Checked<()> {
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
pub(crate) fn judge(elaboration: &Elaboration) -> Checked<Vec<ExceededReach>> {
    let module = &elaboration.module;
    let mut exceeded = Vec::new();
    if module
        .services
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        let first = MachineId::new(1).expect("1 is nonzero");
        return Err(fault(first, "services are not strictly ordered"));
    }
    for machine in &module.machines {
        let id = machine.id;
        let published = machine.published_service_ceiling.as_slice();
        well_formed(module, id, published)?;
        well_formed(module, id, &machine.declared_service_reach)?;
        let ceiling = elaboration
            .reach
            .ceilings
            .get(id)
            .and_then(|ceiling| ceiling.as_deref());
        match ceiling {
            Some(ceiling) => {
                if machine.declared_service_reach != ceiling {
                    return Err(fault(id, "the declared reach is not the written ceiling"));
                }
            }
            None if !machine.declared_service_reach.is_empty() => {
                return Err(fault(
                    id,
                    "a reach is declared where no ceiling was written",
                ));
            }
            None => {}
        }
        // A call through a callable value contributes what its candidates
        // publish, which this judgment does not union.
        if !module.services.is_empty()
            && elaboration.tables.machines.get(id).is_some_and(|tables| {
                tables
                    .invocations
                    .values()
                    .any(|row| matches!(row.callee, Callee::Selected { .. }))
            })
        {
            return Err(refusal(
                RefusalReason::Unsupported,
                id,
                Construct::Machine,
                "machine service reach through a selected callable",
            ));
        }
        for invocation in machine.module_invocations() {
            let callee = invocation.inputs.callee;
            let contributed = machine_at(module, callee)
                .ok_or_else(|| fault(id, "a call names a machine outside the module"))?
                .published_service_ceiling
                .as_slice();
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
            let (at, site) = match invocation.site {
                InvocationSite::Operation(operation) => (
                    Construct::Operation(operation),
                    elaboration
                        .sources
                        .get(id)
                        .and_then(|sources| sources.operations.get(operation)),
                ),
                InvocationSite::ExternalCompletion(_) => {
                    return Err(fault(id, "a module call is an external completion"));
                }
                InvocationSite::InvokeNoNormal(edge) => (
                    Construct::Edge(edge),
                    elaboration
                        .sources
                        .get(id)
                        .and_then(|sources| sources.edges.get(edge)),
                ),
            };
            let site = *site.ok_or_else(|| fault(id, "a call has no source correspondence"))?;
            let statement = format!(
                "machine `{}` publishes service reach `{}` but its checked body reaches \
                 undeclared service{} `{}`: the call of `{}` contributes `{}`",
                machine_name(elaboration, id),
                row_text(elaboration, ceiling),
                if undeclared.len() == 1 { "" } else { "s" },
                row_text(elaboration, &undeclared),
                machine_name(elaboration, callee),
                row_text(elaboration, contributed),
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
        // A machine that wrote a ceiling publishes no more than it.
        if ceiling.is_some_and(|ceiling| !beyond(published, ceiling).is_empty()) {
            return Err(fault(id, "the published reach exceeds the written ceiling"));
        }
    }
    if let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry
        && !module.services.is_empty()
        && machine_at(module, entry).is_some_and(|machine| {
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
