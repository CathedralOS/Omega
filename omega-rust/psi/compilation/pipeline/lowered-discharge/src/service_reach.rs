/*
Whether every call fits the service reach its caller publishes.

`wiki/spec/language/effects.md`: "derived body reach = normalize(contributions
from calls)" and "derived body reach subset-of authored ceiling, when a clause
is present"; "A derived reach that exceeds its ceiling rejects at the call
that contributes the excess." A machine's `published_service_ceiling` is the
row its callers are charged: its written ceiling when it has one, else what
its body's calls contribute. So one containment decides both cases, and it is
judged per call: what the callee publishes must lie inside what the caller
publishes. The callee's published row is the only thing read of it: "Boundary
calls ... use their pinned requirement ceilings, not a narrower provider body",
and "A boundary call contributes what its requirement declares and not the
called trait's name".

The judgment is set containment over published rows, so it has no
certificate, like a loan conflict. What the rows must be for the containment
to mean anything is checked first, and a row that is not is a table fault,
never a rejection of the program: each row names declared services in
strictly increasing order and contains its members' parents ("parents(R)
subset-of normalize(R)"), a written ceiling is the published row, and the
module's root reach is its entry's.
*/

use semantic_vocabulary::{InvocationSite, MachineId, ServiceId, TargetMachineCallable};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{CheckTables, Checked, Construct, RefusalReason};
use terminal_psi::TerminalModule;

use crate::refusal;

/// One call whose callee publishes a service its caller does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Excess {
    pub machine: MachineId,
    pub at: Construct,
    pub callee: TargetMachineCallable,
    /// Everything the callee publishes, and the part of it outside the
    /// caller's row. Both strictly increasing; `outside` is never empty.
    pub contributes: Vec<ServiceId>,
    pub outside: Vec<ServiceId>,
}

fn fault(machine: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}

fn row(module: &TerminalModule, machine: MachineId, row: &[ServiceId]) -> Checked<()> {
    if row.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(fault(
            machine,
            "service reach row is not strictly increasing",
        ));
    }
    for service in row {
        let declaration = module
            .services
            .iter()
            .find(|declaration| declaration.id == *service)
            .ok_or_else(|| fault(machine, "service reach row names an undeclared service"))?;
        if declaration
            .parents
            .iter()
            .any(|parent| row.binary_search(parent).is_err())
        {
            return Err(fault(machine, "service reach row omits a boundary parent"));
        }
    }
    Ok(())
}

pub(crate) fn judge(module: &TerminalModule, tables: &CheckTables) -> Checked<Vec<Excess>> {
    for machine in &module.machines {
        row(module, machine.id, &machine.published_service_ceiling)?;
        row(module, machine.id, &machine.declared_service_reach)?;
        if !machine.declared_service_reach.is_empty()
            && machine.declared_service_reach != machine.published_service_ceiling
        {
            return Err(fault(
                machine.id,
                "written service ceiling is not the published row",
            ));
        }
    }
    if let Some(machine) = module.machines.first().map(|machine| machine.id) {
        for boundary in &module.boundary_machines {
            row(module, machine, &boundary.published_service_ceiling)?;
            row(module, machine, &boundary.fixed_service_reach)?;
            if boundary.fixed_service_reach.iter().any(|service| {
                boundary
                    .published_service_ceiling
                    .binary_search(service)
                    .is_err()
            }) {
                return Err(fault(
                    machine,
                    "boundary fixed reach is outside its published ceiling",
                ));
            }
        }
    }
    if let Some(TargetMachineCallable::Machine(entry)) = module.entry {
        let published = module
            .machine(entry)
            .map(|machine| machine.published_service_ceiling.as_slice());
        if published != Some(module.root_service_reach.concrete.as_slice()) {
            return Err(fault(
                entry,
                "root service reach is not the entry's published row",
            ));
        }
    }
    let mut found = Vec::new();
    for caller in &module.machines {
        let rows = tables
            .machines
            .get(caller.id)
            .ok_or_else(|| fault(caller.id, "machine has no check tables"))?;
        for (invocation, call) in rows.invocations.iter() {
            let concerns = Construct::Invocation(invocation);
            let callee = match call.callee {
                Callee::Declared(callee) => callee,
                // A selector's candidates each publish a row; no slice
                // charges a caller for them yet.
                Callee::Selected { .. } if module.services.is_empty() => continue,
                Callee::Selected { .. } => {
                    return Err(refusal(
                        RefusalReason::Unsupported,
                        caller.id,
                        concerns,
                        "service reach of a call through a selector value",
                    ));
                }
            };
            let contributes = match callee {
                TargetMachineCallable::Machine(callee) => module
                    .machine(callee)
                    .map(|callee| callee.published_service_ceiling.as_slice()),
                TargetMachineCallable::Boundary(callee) => module
                    .boundary_machine(callee)
                    .map(|callee| callee.published_service_ceiling.as_slice()),
            }
            .ok_or_else(|| {
                refusal(
                    RefusalReason::ForeignIdentity,
                    caller.id,
                    concerns,
                    "callee declaration is unavailable",
                )
            })?;
            let outside: Vec<_> = contributes
                .iter()
                .copied()
                .filter(|service| {
                    caller
                        .published_service_ceiling
                        .binary_search(service)
                        .is_err()
                })
                .collect();
            if !outside.is_empty() {
                found.push(Excess {
                    machine: caller.id,
                    at: match call.site {
                        InvocationSite::Operation(operation) => Construct::Operation(operation),
                        InvocationSite::ExternalCompletion(edge)
                        | InvocationSite::InvokeNoNormal(edge) => Construct::Edge(edge),
                    },
                    callee,
                    contributes: contributes.to_vec(),
                    outside,
                });
            }
        }
    }
    Ok(found)
}
