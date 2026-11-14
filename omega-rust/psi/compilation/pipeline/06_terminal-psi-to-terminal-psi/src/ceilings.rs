/*
Whether every call fits the ceilings its caller publishes: service reach
(`reaches`) and synchronous entries (`invokes`).

Both axes are one judgment. A machine publishes a row of service identities
on each: its written ceiling when it has one, else what its body's calls
contribute. So one containment decides the written and the derived case, and
it is judged per call: what the callee publishes must lie inside what the
caller publishes. The callee's published row is the only thing read of it.

`wiki/spec/language/effects.md`, for reach: "derived body reach =
normalize(contributions from calls)" and "derived body reach subset-of
authored ceiling, when a clause is present"; "A derived reach that exceeds
its ceiling rejects at the call that contributes the excess."; "Boundary
calls ... use their pinned requirement ceilings, not a narrower provider
body", and "A boundary call contributes what its requirement declares and not
the called trait's name". For entries ("Direct synchronous invocation"): a
written clause on a checked body is to "Bound the derived entries", and "A
call to a `ProgramToProvider` slot is not itself an entry. Entry demands
reached through it still propagate: the callee's published `invokes` joins
the caller's, as its reach does."

The judgment is set containment over published rows, so it has no
certificate, like a loan conflict. What the rows must be for the containment
to mean anything is checked first, and a row that is not is a table fault,
never a rejection of the program: each row names declared services in
strictly increasing order; a reach row contains its members' parents
("parents(R) subset-of normalize(R)"; an entry row has no parent closure), a
written reach ceiling is the published row, and the module's root reach is
its entry's.

No operation of the lowered form originates an entry yet: a call of a
callable argument, of a stored callable or of an inbound entry would add its
own identity at that call, and the route reads none of them. A call through
a selector value is refused in a module that declares services.
*/

use semantic_vocabulary::{InvocationSite, MachineId, ServiceId, TargetMachineCallable};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{CheckTables, Checked, Construct, RefusalReason};
use terminal_psi::{BoundaryMachineDeclaration, TerminalMachine, TerminalModule};

use crate::refusal;

/// The two ceilings a machine publishes as a row of services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// `reaches`: closed under boundary parents.
    Reach,
    /// `invokes`: the requirement identities of the entries control may
    /// arrive at before the machine returns. No parent closure.
    Entries,
}

impl Axis {
    const ALL: [Axis; 2] = [Axis::Reach, Axis::Entries];

    fn of_machine(self, machine: &TerminalMachine) -> &[ServiceId] {
        match self {
            Axis::Reach => &machine.published_service_ceiling,
            Axis::Entries => &machine.published_synchronous_entries,
        }
    }

    fn of_boundary(self, boundary: &BoundaryMachineDeclaration) -> &[ServiceId] {
        match self {
            Axis::Reach => &boundary.published_service_ceiling,
            Axis::Entries => &boundary.published_synchronous_entries,
        }
    }
}

/// One call whose callee publishes a service its caller does not, on `axis`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Excess {
    pub axis: Axis,
    pub machine: MachineId,
    pub at: Construct,
    pub callee: TargetMachineCallable,
    /// Everything the callee publishes on the axis, and the part of it
    /// outside the caller's row. Both strictly increasing; `outside` is
    /// never empty.
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

fn row(module: &TerminalModule, machine: MachineId, axis: Axis, row: &[ServiceId]) -> Checked<()> {
    if row.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(fault(machine, "published row is not strictly increasing"));
    }
    for service in row {
        let declaration = module
            .services
            .iter()
            .find(|declaration| declaration.id == *service)
            .ok_or_else(|| fault(machine, "published row names an undeclared service"))?;
        if axis == Axis::Reach
            && declaration
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
        for axis in Axis::ALL {
            row(module, machine.id, axis, axis.of_machine(machine))?;
        }
        row(
            module,
            machine.id,
            Axis::Reach,
            machine
                .declared_service_reach
                .as_deref()
                .unwrap_or_default(),
        )?;
        if machine
            .declared_service_reach
            .as_ref()
            .is_some_and(|reach| *reach != machine.published_service_ceiling)
        {
            return Err(fault(
                machine.id,
                "written service ceiling is not the published row",
            ));
        }
    }
    if let Some(machine) = module.machines.first().map(|machine| machine.id) {
        for boundary in &module.boundary_machines {
            for axis in Axis::ALL {
                row(module, machine, axis, axis.of_boundary(boundary))?;
            }
            row(module, machine, Axis::Reach, &boundary.fixed_service_reach)?;
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
            let at = match call.site {
                InvocationSite::Operation(operation) => Construct::Operation(operation),
                InvocationSite::ExternalCompletion(edge) | InvocationSite::InvokeNoNormal(edge) => {
                    Construct::Edge(edge)
                }
            };
            contribution(module, caller, callee, at, &mut found)?;
        }
        for formation in &caller.call_formations {
            if let (terminal_psi::CallSite::Edge(edge), Some(callee)) =
                (formation.site, formation.tail_callee)
            {
                contribution(
                    module,
                    caller,
                    callee.into(),
                    Construct::Edge(edge),
                    &mut found,
                )?;
            }
        }
    }
    Ok(found)
}

/// Frame reuse changes execution into a jump, not the published interface
/// charged to the caller. Every retained call uses the same containment.
fn contribution(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: TargetMachineCallable,
    at: Construct,
    found: &mut Vec<Excess>,
) -> Checked<()> {
    for axis in Axis::ALL {
        let contributes = match callee {
            TargetMachineCallable::Machine(id) => {
                module.machine(id).map(|machine| axis.of_machine(machine))
            }
            TargetMachineCallable::Boundary(id) => module
                .boundary_machine(id)
                .map(|boundary| axis.of_boundary(boundary)),
        }
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                caller.id,
                at,
                "callee declaration is unavailable",
            )
        })?;
        let ceiling = axis.of_machine(caller);
        let outside: Vec<_> = contributes
            .iter()
            .copied()
            .filter(|service| ceiling.binary_search(service).is_err())
            .collect();
        if !outside.is_empty() {
            found.push(Excess {
                axis,
                machine: caller.id,
                at,
                callee,
                contributes: contributes.to_vec(),
                outside,
            });
        }
    }
    Ok(())
}
