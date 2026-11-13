//! Publish one canonical service namespace and each call's public reach.
//!
//! Source envelopes retain source service identities until the complete used
//! closure is known. Only ServiceMap converts them to hermetically ordered
//! Terminal identities. The receiving checker independently judges every call.

use language_semantics::ServiceReachId;
use semantic_vocabulary::{IdTable, MachineId, ServiceId};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::{
    ServiceDeclaration, TerminalCallableEnvelope, TerminalCallableInvocation,
    TerminalCallableTermination, TerminalModule,
};
use typed_trees::TypedTrees;

use crate::{Elaboration, Unserved, refuse};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceInvocation {
    Parameter(u32),
    Service(ServiceReachId),
}

impl Ord for SourceInvocation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let key = |target: &Self| match target {
            Self::Parameter(position) => (0u8, *position),
            Self::Service(service) => (1, service.0),
        };
        key(self).cmp(&key(other))
    }
}
impl PartialOrd for SourceInvocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// These identities belong to frozen typed trees, not a Terminal namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceEnvelope {
    pub service_reach: Vec<ServiceReachId>,
    pub invocations: Vec<SourceInvocation>,
    pub suspends: bool,
    pub blocks: bool,
    pub termination: TerminalCallableTermination,
}

#[derive(Debug, Clone)]
struct ServiceSource {
    id: ServiceId,
    identity: String,
    name: String,
    parents: Vec<ServiceId>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ServiceMap {
    declarations: Vec<ServiceSource>,
    bindings: Vec<(ServiceReachId, ServiceId)>,
}
impl ServiceMap {
    pub(crate) fn prepare(
        typed: &TypedTrees,
        context: MachineId,
        registry: &IdTable<
            ServiceId,
            (
                typed_trees::trait_definition::AppliedTraitScope<'_>,
                Vec<ServiceId>,
            ),
        >,
        bindings: Vec<(ServiceReachId, ServiceId)>,
    ) -> Checked<Self> {
        let mut declarations = Vec::with_capacity(registry.len());
        for (id, (scope, parents)) in registry.iter() {
            let identity = scope
                .portable_application_identity(typed)
                .and_then(|identity| typed.type_identity_text(identity))
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    unavailable(
                        context,
                        "service has no complete exact application identity",
                    )
                })?;
            if declarations
                .iter()
                .any(|known: &ServiceSource| known.identity == identity)
            {
                return Err(unavailable(
                    context,
                    "distinct source services have one application identity",
                ));
            }
            declarations.push(ServiceSource {
                id,
                identity,
                name: scope.declaration.name.as_str().to_owned(),
                parents: parents.clone(),
            });
        }
        for (_, id) in &bindings {
            if !declarations.iter().any(|row| row.id == *id) {
                return Err(unavailable(
                    context,
                    "source service binding has no exact registry row",
                ));
            }
        }
        Ok(Self {
            declarations,
            bindings,
        })
    }
    pub(crate) fn service(&self, context: MachineId, source: ServiceReachId) -> Checked<ServiceId> {
        self.bindings
            .iter()
            .find_map(|(known, id)| (*known == source).then_some(*id))
            .ok_or_else(|| {
                unavailable(context, "service is outside the retained canonical closure")
            })
    }
    pub(crate) fn identity(&self, service: ServiceId) -> Option<&str> {
        self.declarations
            .iter()
            .find(|source| source.id == service)
            .map(|source| source.identity.as_str())
    }
    pub(crate) fn row(
        &self,
        context: MachineId,
        sources: &[ServiceReachId],
    ) -> Checked<Vec<ServiceId>> {
        let mut row = sources
            .iter()
            .map(|source| self.service(context, *source))
            .collect::<Checked<Vec<_>>>()?;
        row.sort();
        row.dedup();
        Ok(row)
    }
    pub(crate) fn envelope(
        &self,
        context: MachineId,
        source: &SourceEnvelope,
    ) -> Checked<TerminalCallableEnvelope> {
        let mut invocations = source
            .invocations
            .iter()
            .map(|target| match target {
                SourceInvocation::Parameter(position) => {
                    Ok(TerminalCallableInvocation::Parameter(*position))
                }
                SourceInvocation::Service(service) => self
                    .service(context, *service)
                    .map(TerminalCallableInvocation::Service),
            })
            .collect::<Checked<Vec<_>>>()?;
        invocations.sort();
        invocations.dedup();
        Ok(TerminalCallableEnvelope {
            service_reach: self.row(context, &source.service_reach)?,
            invocations,
            suspends: source.suspends,
            blocks: source.blocks,
            termination: source.termination.clone(),
        })
    }
    fn publish(&self, context: MachineId, module: &mut TerminalModule) -> Checked<Vec<String>> {
        if !module.services.is_empty() {
            return Err(unavailable(
                context,
                "service namespace was already published",
            ));
        }
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let mut names = Vec::with_capacity(self.declarations.len());
        for source in &self.declarations {
            module.services.push(ServiceDeclaration {
                id: source.id,
                identity: identities.intern(&source.identity),
                parents: source.parents.clone(),
            });
            names.push(source.name.clone());
        }
        module.identities = identities.build();
        Ok(names)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServiceReach {
    /// Only this authored/omitted distinction is extra request information.
    /// Calls and their contributed rows are read back from the lowered module.
    pub ceilings: IdTable<MachineId, Option<Vec<ServiceId>>>,
    pub names: Vec<String>,
}
fn unavailable(context: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refuse(context, Construct::Machine, Unserved::Machine(detail))
}

pub(crate) fn source_closure(
    typed: &TypedTrees,
    context: MachineId,
    roots: &[ServiceReachId],
) -> Checked<Vec<ServiceReachId>> {
    let mut pending = roots.iter().map(|id| (*id, false)).collect::<Vec<_>>();
    let mut active = Vec::new();
    let mut completed = Vec::new();
    while let Some((id, exiting)) = pending.pop() {
        let definition = typed
            .service_reaches
            .definition(id)
            .ok_or_else(|| unavailable(context, "service reach names an unknown declaration"))?;
        if exiting {
            if active.pop() != Some(id) {
                return Err(unavailable(
                    context,
                    "service closure lost its active declaration",
                ));
            }
            completed.push(id);
            continue;
        }
        if active.contains(&id) {
            return Err(unavailable(context, "service inheritance has a cycle"));
        }
        if completed.contains(&id) {
            continue;
        }
        active.push(id);
        pending.push((id, true));
        pending.extend(definition.parents.iter().map(|parent| (*parent, false)));
    }
    completed.sort_by_key(|id| id.0);
    completed.dedup();
    Ok(completed)
}

/// Writtenness comes from the exact source declaration, including a closed
/// requirement's retained template. A normalized empty row cannot supply it.
pub(crate) fn has_authored_ceiling(
    typed: &TypedTrees,
    context: MachineId,
    symbol: symbols::SymbolHandle,
) -> Checked<bool> {
    authored_ceiling_owner(typed, context, symbol).map(|owner| owner.is_some())
}

pub(crate) fn authored_ceiling_owner(
    typed: &TypedTrees,
    context: MachineId,
    symbol: symbols::SymbolHandle,
) -> Checked<Option<symbols::SymbolHandle>> {
    let mut current = symbol;
    let mut visited = Vec::new();
    loop {
        if !typed.symbols.symbols().nodes().is_valid(current) || visited.contains(&current) {
            return Err(unavailable(
                context,
                "service ceiling has stale or cyclic declaration provenance",
            ));
        }
        visited.push(current);
        if typed
            .authored_service_reach_rows_for(current)
            .next()
            .is_some()
        {
            return Ok(Some(current));
        }
        if let Some(application) = typed.closed_requirement_application(current) {
            current = application.template;
            continue;
        }
        if let Some(machine) = typed.machine_by_own_or_state_symbol(current) {
            if machine.symbol != current {
                current = machine.symbol;
                continue;
            }
            if let Some(template) = crate::template_of(typed, current) {
                current = template;
                continue;
            }
        }
        return Ok(None);
    }
}

/// An interned empty row also represents an omitted clause. Only authored
/// provenance (including a generated machine's template) establishes a promise.
pub(crate) fn written_ceiling(
    typed: &TypedTrees,
    context: MachineId,
    symbol: symbols::SymbolHandle,
) -> Checked<Option<Vec<ServiceReachId>>> {
    let machine = typed
        .machine_by_symbol(symbol)
        .ok_or_else(|| unavailable(context, "reach header has no exact machine declaration"))?;
    let written = has_authored_ceiling(typed, context, symbol)?;
    if !written {
        return Ok(None);
    }
    if machine.service_reach_is_installation_bound {
        return Err(unavailable(
            context,
            "service reach remains bound to an installation",
        ));
    }
    crate::read::services::finite_reach(typed, context, machine.service_reach_row).map(Some)
}

/// State canonical rows from actual lowered direct, callable and boundary calls.
/// No acceptance follows from this computation; the receiving judgment checks it.
pub(crate) fn publish(
    typed: &TypedTrees,
    entry: MachineId,
    services: &ServiceMap,
    elaboration: &mut Elaboration,
) -> Checked<()> {
    if !elaboration.reach.ceilings.is_empty() || !elaboration.reach.names.is_empty() {
        return Err(unavailable(
            entry,
            "service request metadata was already published",
        ));
    }
    elaboration.reach.names = services.publish(entry, &mut elaboration.module)?;
    let mut ceilings = Vec::with_capacity(elaboration.module.machines.len());
    for (id, symbol) in elaboration.machines.iter() {
        let written = if elaboration.requirements.contains(&id) {
            None
        } else {
            written_ceiling(typed, id, *symbol)?
        };
        ceilings.push(
            written
                .as_ref()
                .map(|row| services.row(id, row))
                .transpose()?,
        );
    }
    let mut published = ceilings
        .iter()
        .map(|ceiling| ceiling.clone().unwrap_or_default())
        .collect::<Vec<_>>();
    let position = |machine: MachineId| {
        usize::try_from(machine.get() - 1)
            .map_err(|_| unavailable(entry, "machine coordinate exceeds host width"))
    };
    loop {
        let mut changed = false;
        for machine in &elaboration.module.machines {
            let caller = position(machine.id)?;
            if ceilings
                .get(caller)
                .ok_or_else(|| unavailable(machine.id, "machine has no authored reach metadata"))?
                .is_some()
            {
                continue;
            }
            let mut contributed = Vec::new();
            for call in machine.module_invocations() {
                let row = published
                    .get(position(call.inputs.callee)?)
                    .ok_or_else(|| {
                        unavailable(machine.id, "reach call names a machine outside the module")
                    })?;
                union(&mut contributed, row);
            }
            for call in machine.callable_invocations() {
                let contract = elaboration
                    .module
                    .callables
                    .contract(call.inputs.contract)
                    .ok_or_else(|| {
                        unavailable(machine.id, "reach call lost its retained callable contract")
                    })?;
                union(&mut contributed, &contract.envelope.service_reach);
            }
            for call in machine.boundary_invocations() {
                let boundary = elaboration
                    .module
                    .boundary_machine(call.boundary)
                    .ok_or_else(|| {
                        unavailable(
                            machine.id,
                            "reach call lost its retained boundary declaration",
                        )
                    })?;
                union(&mut contributed, &boundary.published_service_ceiling);
            }
            let row = published
                .get_mut(caller)
                .ok_or_else(|| unavailable(machine.id, "caller lost its published reach row"))?;
            let before = row.len();
            union(row, &contributed);
            changed |= row.len() != before;
        }
        if !changed {
            break;
        }
    }
    for machine in &mut elaboration.module.machines {
        let at = position(machine.id)?;
        let ceiling = ceilings
            .get(at)
            .ok_or_else(|| unavailable(machine.id, "machine lost its reach ceiling"))?;
        let row = published
            .get(at)
            .ok_or_else(|| unavailable(machine.id, "machine lost its published reach"))?;
        machine.declared_service_reach = ceiling.clone().unwrap_or_default();
        machine.published_service_ceiling = row.clone();
        if let Some(named) = elaboration
            .module
            .callables
            .machine_envelopes
            .get_mut(at)
            .and_then(Option::as_mut)
        {
            if named.machine != machine.id || named.envelope.service_reach != *row {
                return Err(unavailable(
                    machine.id,
                    "source and lowered reach projections disagree",
                ));
            }
        }
    }
    elaboration.module.root_service_reach.concrete = published
        .get(position(entry)?)
        .ok_or_else(|| unavailable(entry, "program entry has no published reach"))?
        .clone();
    for ceiling in ceilings {
        elaboration.reach.ceilings.push(ceiling);
    }
    Ok(())
}
fn union<T: Ord + Clone>(to: &mut Vec<T>, from: &[T]) {
    to.extend_from_slice(from);
    to.sort();
    to.dedup();
}
