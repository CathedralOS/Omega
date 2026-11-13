/*
Writes each machine's service reach into the lowered module: the services the
module names, the ceiling a header wrote, and the row each machine publishes
to its callers. Nothing is judged here. The checker reads these rows back
from the module and refuses a call that contributes more than its caller's
ceiling (`lowered-discharge`, `service_reach`).

`wiki/spec/language/effects.md`, "Declarations": "A present clause is an
upper bound, not an assertion of exact equality and not an additive
contribution. The complete derived reach must fit its normalized ceiling."
and "A checked body may omit `reaches` ... Its reach is then inferred from
every checked call and pinned requirement contract."

So a machine's derived reach is the union of what its calls contribute, and
the row it publishes is:

- its written ceiling, when the header wrote one: "An explicit published
  ceiling may conservatively remain wider than its checked body";
- its derived reach, when the clause was omitted.

Whether a private machine's written ceiling, rather than its derived reach,
is what its callers in the same package see is not stated by the
specification. The wider row is taken, which can only reject more.

Only direct calls of machines contribute here: a boundary operation, a
requirement that declares a reach and a call through a callable value are
refused before this runs or below, so every row is a union of written
ceilings. A module in which no header names a service is left exactly as it
was lowered.

Each service's identity is the hermetic declaration identity the typed trees
fix for its symbol; services are ordered by that identity, as
`wiki/spec/terminal-psi/encoding.md` requires ("Services are strictly ordered
by service id with strictly ordered parents").
*/

use language_semantics::{ServiceReachId, ServiceReachRowId, ServiceReachRowTable};
use semantic_vocabulary::{IdTable, MachineId, ServiceId};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{Checked, Construct};
use terminal_psi::ServiceDeclaration;
use typed_trees::TypedTrees;

use crate::{Elaboration, Unserved, refuse};

/// What elaboration knows of service reach beyond the module's own rows.
#[derive(Debug, Clone, Default)]
pub struct ServiceReach {
    /// The ceiling each machine's header wrote, closed under boundary
    /// parents and in ascending order; `None` when the clause was omitted.
    /// `reaches none` is `Some` of no service, which the module's
    /// `declared_service_reach` cannot tell from an omitted clause. Empty
    /// for bodies lowered without typed trees: none wrote a ceiling.
    pub ceilings: IdTable<MachineId, Option<Vec<ServiceId>>>,
    /// What a diagnostic calls each service, in `ServiceId` order.
    pub names: Vec<String>,
}

fn unserved<T>(machine: MachineId, what: &'static str) -> Checked<T> {
    Err(refuse(machine, Construct::Machine, Unserved::Machine(what)))
}

/// The ceiling `symbol`'s header wrote, as typed-tree services closed under
/// parents. A machine stage 04 generated from a template has the template's
/// clause.
fn written_ceiling(
    typed: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<Option<Vec<ServiceReachId>>> {
    let machine = typed.machine_by_symbol(symbol)?;
    let authored = |owner| {
        typed
            .authored_service_reach_rows_for(owner)
            .next()
            .is_some()
    };
    let written = authored(symbol)
        || crate::template_of(typed, symbol).is_some_and(authored)
        || !matches!(
            machine.service_reach_row,
            ServiceReachRowId::NULL | ServiceReachRowTable::EMPTY_ROW
        );
    let ceiling = written.then(|| {
        let mut closed = Vec::new();
        for service in typed.service_reach_rows.services(machine.service_reach_row) {
            typed.service_reaches.extend_closure(*service, &mut closed);
        }
        closed
    });
    Some(ceiling)
}

/// States the reach rows of every machine of `elaboration`, whose machines
/// are the typed-tree machines `elaboration.machines` lists.
pub(crate) fn publish(
    typed: &TypedTrees,
    entry: MachineId,
    elaboration: &mut Elaboration,
) -> Checked<()> {
    let mut written: Vec<Option<Vec<ServiceReachId>>> = Vec::new();
    for (id, symbol) in elaboration.machines.iter() {
        written.push(if elaboration.requirements.contains(&id) {
            None
        } else {
            written_ceiling(typed, *symbol).flatten()
        });
    }
    let mut used: Vec<ServiceReachId> = Vec::new();
    for ceiling in &written {
        for service in ceiling.iter().flatten() {
            if !used.contains(service) {
                used.push(*service);
            }
        }
    }
    if used.is_empty() {
        // `reaches none` still bounds its body; no service is declared.
        for ceiling in written {
            elaboration.reach.ceilings.push(ceiling.map(|_| Vec::new()));
        }
        return Ok(());
    }
    // A call through a callable value reaches what its candidates publish,
    // which this pass does not union.
    for (id, tables) in elaboration.tables.machines.iter() {
        if tables
            .invocations
            .values()
            .any(|row| matches!(row.callee, Callee::Selected { .. }))
        {
            return unserved(id, "service reach through a selected callable");
        }
    }

    let mut identified = Vec::with_capacity(used.len());
    for service in &used {
        let Some(definition) = typed.service_reaches.definition(*service) else {
            return unserved(entry, "service reach names an unknown service");
        };
        let Ok(identity) = typed.normalized_hermetic_symbol_identity(definition.symbol) else {
            return Err(refuse(
                entry,
                Construct::Machine,
                Unserved::Spelling("service without a hermetic identity"),
            ));
        };
        identified.push((*service, identity, definition));
    }
    identified.sort_by(|left, right| left.1.cmp(&right.1));
    if identified
        .windows(2)
        .any(|pair| pair[0].1 == pair[1].1 || pair[0].1.is_empty())
    {
        return unserved(entry, "services with one identity");
    }
    let lowered = |service: ServiceReachId| -> Checked<ServiceId> {
        identified
            .iter()
            .position(|(candidate, _, _)| *candidate == service)
            .and_then(|position| ServiceId::new(position as u64 + 1))
            .ok_or_else(|| {
                refuse(
                    entry,
                    Construct::Machine,
                    Unserved::Machine("service parent outside the closure"),
                )
            })
    };
    let row = |services: &[ServiceReachId]| -> Checked<Vec<ServiceId>> {
        let mut row = services
            .iter()
            .map(|service| lowered(*service))
            .collect::<Checked<Vec<_>>>()?;
        row.sort();
        row.dedup();
        Ok(row)
    };

    let mut identities = std::mem::take(&mut elaboration.module.identities).into_builder();
    let mut declarations = Vec::with_capacity(identified.len());
    for (service, identity, definition) in &identified {
        declarations.push(ServiceDeclaration {
            id: lowered(*service)?,
            identity: identities.intern(identity),
            parents: row(&definition.parents)?,
        });
        elaboration.reach.names.push(definition.name.clone());
    }
    elaboration.module.identities = identities.build();
    elaboration.module.services = declarations;

    let mut ceilings = Vec::with_capacity(written.len());
    for ceiling in &written {
        ceilings.push(match ceiling {
            Some(services) => Some(row(services)?),
            None => None,
        });
    }
    // What each machine publishes: a written ceiling as it stands, and for
    // an omitted clause the least row closed over the machine's direct calls.
    let mut published: Vec<Vec<ServiceId>> = ceilings
        .iter()
        .map(|ceiling| ceiling.clone().unwrap_or_default())
        .collect();
    let fixed: Vec<bool> = ceilings.iter().map(Option::is_some).collect();
    let position = |machine: MachineId| (machine.get() - 1) as usize;
    let mut changed = true;
    while changed {
        changed = false;
        for machine in &elaboration.module.machines {
            let caller = position(machine.id);
            if fixed.get(caller) != Some(&false) {
                continue;
            }
            for invocation in machine.module_invocations() {
                let Some(contributed) = published.get(position(invocation.inputs.callee)).cloned()
                else {
                    return unserved(machine.id, "call of a machine outside the module");
                };
                for service in contributed {
                    if let Err(at) = published[caller].binary_search(&service) {
                        published[caller].insert(at, service);
                        changed = true;
                    }
                }
            }
        }
    }
    for machine in &mut elaboration.module.machines {
        let at = position(machine.id);
        let (Some(ceiling), Some(row)) = (ceilings.get(at), published.get(at)) else {
            return unserved(machine.id, "machine without a typed-tree declaration");
        };
        machine.declared_service_reach = ceiling.clone().unwrap_or_default();
        machine.published_service_ceiling = row.clone();
    }
    if let Some(root) = published.get(position(entry)) {
        elaboration.module.root_service_reach.concrete = root.clone();
    }
    for ceiling in ceilings {
        elaboration.reach.ceilings.push(ceiling);
    }
    Ok(())
}
