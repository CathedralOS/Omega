//! Calls must fit the permissions their callers publish. Service reach and
//! synchronous entries share one owner, while retaining parameter identities.
//! A callable's type bounds what it may do; invoking its holder also enters
//! that exact formal, even when the type itself enters nothing further.

use semantic_vocabulary::{
    CallableContractId, InvocationSite, MachineId, ServiceId, TargetMachineCallable,
};
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{
    BoundaryParameterKind, TerminalCallableEnvelope, TerminalCallableInvocation as Entry,
    TerminalMachine, TerminalModule,
};
use terminal_semantics::callable_values::{
    CallableFormalInput, CallableInvocationSite, CallableParameter, CallableValuePossibilities,
    CallableValueProgram, CallableValueQuery, resolve_callable_value, terminal_callable_values,
};

use crate::refusal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Reach,
    Entries,
    Suspension,
    Blocking,
    Termination,
}
impl Axis {
    const ALL: [Self; 5] = [
        Self::Reach,
        Self::Entries,
        Self::Suspension,
        Self::Blocking,
        Self::Termination,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Permission {
    Entry(Entry),
    MaySuspend,
    MayBlock,
    MayDiverge,
}

#[derive(Clone, Copy)]
enum Envelope<'a> {
    Runtime(&'a TerminalCallableEnvelope),
    /// These source-only signatures are not encoded, sealed or executable.
    /// An omitted clause remains Open, never a closed empty runtime row.
    Abstract(&'a terminal_check_tables::contracts::RequirementCeilings),
}
impl<'a> Envelope<'a> {
    fn reach(self) -> Option<&'a [ServiceId]> {
        match self {
            Self::Runtime(envelope) => Some(&envelope.service_reach),
            Self::Abstract(envelope) => match &envelope.reaches {
                terminal_check_tables::contracts::Ceiling::Inferred => None,
                terminal_check_tables::contracts::Ceiling::Authored(row) => Some(row),
            },
        }
    }
    fn entries(self) -> Option<Vec<Entry>> {
        match self {
            Self::Runtime(envelope) => Some(envelope.invocations.clone()),
            Self::Abstract(envelope) => match &envelope.invokes {
                terminal_check_tables::contracts::Ceiling::Inferred => None,
                terminal_check_tables::contracts::Ceiling::Authored(row) => {
                    Some(row.iter().copied().map(Entry::Service).collect())
                }
            },
        }
    }
    fn row(self, axis: Axis) -> Option<Vec<Permission>> {
        match axis {
            Axis::Reach => self.reach().map(|row| {
                row.iter()
                    .copied()
                    .map(Entry::Service)
                    .map(Permission::Entry)
                    .collect()
            }),
            Axis::Entries => self
                .entries()
                .map(|row| row.into_iter().map(Permission::Entry).collect()),
            Axis::Suspension | Axis::Blocking | Axis::Termination => {
                let Self::Runtime(envelope) = self else {
                    return None;
                };
                let permitted = match axis {
                    Axis::Suspension => envelope.suspends.then_some(Permission::MaySuspend),
                    Axis::Blocking => envelope.blocks.then_some(Permission::MayBlock),
                    Axis::Termination => matches!(
                        envelope.termination,
                        terminal_psi::TerminalCallableTermination::NotGuaranteed
                    )
                    .then_some(Permission::MayDiverge),
                    _ => unreachable!(),
                };
                Some(permitted.into_iter().collect())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Excess {
    pub axis: Axis,
    pub machine: MachineId,
    pub at: Construct,
    pub callee: Callee,
    pub ceiling: Vec<Permission>,
    pub contributes: Vec<Permission>,
    pub outside: Vec<Permission>,
}

fn fault(machine: MachineId, detail: &'static str) -> terminal_check_tables::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}

/// Canonical service rows are ordered, declared and closed under parents.
/// Static header admission uses this same operation as executable calls.
pub(crate) fn well_formed_reach(
    module: &TerminalModule,
    machine: MachineId,
    row: &[ServiceId],
) -> Checked<()> {
    service_row(module, machine, row, true)
}

fn service_row(
    module: &TerminalModule,
    machine: MachineId,
    row: &[ServiceId],
    parents: bool,
) -> Checked<()> {
    if row.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(fault(
            machine,
            "published service row is not strictly increasing",
        ));
    }
    for service in row {
        let declaration = module
            .services
            .iter()
            .find(|declaration| declaration.id == *service)
            .ok_or_else(|| fault(machine, "published row names an undeclared service"))?;
        if parents
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

fn services(envelope: &TerminalCallableEnvelope) -> Vec<ServiceId> {
    envelope
        .invocations
        .iter()
        .filter_map(|entry| match entry {
            Entry::Service(service) => Some(*service),
            Entry::Parameter(_) => None,
        })
        .collect()
}

fn machine_envelope<'a>(
    elaboration: &'a terminal_check_tables::program::Elaboration,
    machine: MachineId,
) -> Checked<Envelope<'a>> {
    if let Some(envelope) = elaboration.module.callables.machine_envelope(machine) {
        return Ok(Envelope::Runtime(envelope));
    }
    if !elaboration.requirements.is_empty()
        && elaboration.module.callables.contracts.is_empty()
        && elaboration.module.callables.descriptors.is_empty()
        && let Some(envelope) = elaboration.requirement_ceilings.get(machine)
    {
        return Ok(Envelope::Abstract(envelope));
    }
    Err(fault(
        machine,
        "named machine has no exact complete operational envelope",
    ))
}

fn envelope_for<'a>(
    elaboration: &'a terminal_check_tables::program::Elaboration,
    caller: MachineId,
    inputs: crate::call_contracts::Inputs<'_>,
) -> Checked<Envelope<'a>> {
    let module = &elaboration.module;
    match inputs.callee {
        Callee::Declared(TargetMachineCallable::Machine(machine)) => {
            machine_envelope(elaboration, machine)
        }
        Callee::Declared(TargetMachineCallable::Boundary(boundary)) => module
            .callables
            .boundary_envelope(boundary)
            .map(Envelope::Runtime)
            .ok_or_else(|| {
                fault(
                    caller,
                    "boundary invocation has no complete operational envelope",
                )
            }),
        Callee::Selected { .. } => inputs
            .contract
            .and_then(|contract| module.callables.contract(contract))
            .map(|contract| Envelope::Runtime(&contract.envelope))
            .ok_or_else(|| {
                fault(
                    caller,
                    "selected invocation has no complete operational envelope",
                )
            }),
    }
}

fn union<T: Ord>(into: &mut Vec<T>, values: impl IntoIterator<Item = T>) {
    into.extend(values);
    into.sort_unstable();
    into.dedup();
}

#[derive(Default)]
struct Contributions {
    reach: Vec<ServiceId>,
    entries: Vec<Entry>,
    suspends: bool,
    blocks: bool,
    may_diverge: bool,
    conditional_progress: bool,
}

impl Contributions {
    fn operational(&mut self, envelope: &TerminalCallableEnvelope) {
        self.suspends |= envelope.suspends;
        self.blocks |= envelope.blocks;
        match &envelope.termination {
            terminal_psi::TerminalCallableTermination::NotGuaranteed => self.may_diverge = true,
            terminal_psi::TerminalCallableTermination::Terminates { premises } => {
                self.conditional_progress |= !premises.is_empty();
            }
        }
    }
    fn row(&self, axis: Axis) -> Vec<Permission> {
        match axis {
            Axis::Reach => self
                .reach
                .iter()
                .copied()
                .map(Entry::Service)
                .map(Permission::Entry)
                .collect(),
            Axis::Entries => self
                .entries
                .iter()
                .copied()
                .map(Permission::Entry)
                .collect(),
            Axis::Suspension => self
                .suspends
                .then_some(Permission::MaySuspend)
                .into_iter()
                .collect(),
            Axis::Blocking => self
                .blocks
                .then_some(Permission::MayBlock)
                .into_iter()
                .collect(),
            Axis::Termination => self
                .may_diverge
                .then_some(Permission::MayDiverge)
                .into_iter()
                .collect(),
        }
    }
}

struct Calls<'a> {
    module: &'a TerminalModule,
    values: Option<&'a CallableValueProgram<'a>>,
    caller: &'a TerminalMachine,
    at: Construct,
}
impl Calls<'_> {
    fn foreign(&self, detail: &'static str) -> terminal_check_tables::Refusal {
        refusal(
            RefusalReason::ForeignIdentity,
            self.caller.id,
            self.at,
            detail,
        )
    }
    fn unsupported(&self, detail: &'static str) -> terminal_check_tables::Refusal {
        refusal(RefusalReason::Unsupported, self.caller.id, self.at, detail)
    }
    fn contract(&self, id: CallableContractId) -> Checked<&terminal_psi::TerminalCallableContract> {
        self.module
            .callables
            .contract(id)
            .ok_or_else(|| self.foreign("invoked callable lost its exact contract"))
    }
    fn resolve(&self, query: CallableValueQuery) -> Checked<CallableValuePossibilities> {
        let values = self.values.ok_or_else(|| {
            self.foreign("callable origins were not prepared for this invocation")
        })?;
        resolve_callable_value(values, query).map_err(|reason| self.foreign(reason))
    }
    fn formal(&self, input: &CallableFormalInput) -> Checked<Entry> {
        if input.machine != self.caller.id {
            return Err(
                self.foreign("invoked value retained another machine's unsubstituted formal")
            );
        }
        let (kind, position) = match &input.parameter {
            CallableParameter::Scalar(position) => {
                if self
                    .caller
                    .parameters
                    .get(*position)
                    .map(|formal| formal.scalar_type)
                    != Some(semantic_vocabulary::ScalarType::Callable(input.contract))
                {
                    return Err(
                        self.foreign("invoked scalar origin disagrees with its formal contract")
                    );
                }
                (BoundaryParameterKind::Scalar, *position)
            }
            CallableParameter::Structural { position, path } => {
                if !path.is_empty() {
                    return Err(self
                        .unsupported("invokes needs the exact nested structural parameter path"));
                }
                if self.caller.structural_parameters.get(*position).is_none() {
                    return Err(self.foreign("invoked structural origin names no actual formal"));
                }
                (BoundaryParameterKind::Structural, *position)
            }
        };
        let authored = self
            .caller
            .parameter_order
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == kind)
            .nth(position)
            .map(|(position, _)| position)
            .ok_or_else(|| self.foreign("invoked formal has no authored parameter coordinate"))?;
        Ok(Entry::Parameter(u32::try_from(authored).map_err(|_| {
            self.foreign("invoked formal coordinate is too large")
        })?))
    }
    /// Preserve symbolic formal origins. Concrete descriptors authenticate a
    /// code value; selecting one never inspects its body to narrow the type.
    fn holder(
        &self,
        contract: CallableContractId,
        values: &CallableValuePossibilities,
        reached: &mut Contributions,
    ) -> Checked<()> {
        if values.descriptors.is_empty()
            && values.inputs.is_empty()
            && values.unresolved_ceilings.is_empty()
        {
            return Err(self.foreign("invoked callable has no value origin or explicit ceiling"));
        }
        for descriptor in &values.descriptors {
            let descriptor = self
                .module
                .callables
                .descriptor(*descriptor)
                .ok_or_else(|| {
                    self.foreign("invoked value names no exact constructed descriptor")
                })?;
            if descriptor.contract != contract || self.module.machine(descriptor.machine).is_none()
            {
                return Err(
                    self.foreign("invoked descriptor changes its contract or selected code")
                );
            }
        }
        for input in &values.inputs {
            if input.contract != contract {
                return Err(self.foreign("invoked formal changes its retained contract"));
            }
            union(&mut reached.entries, [self.formal(input)?]);
        }
        for ceiling in &values.unresolved_ceilings {
            self.contract(*ceiling)?;
        }
        if !values.unresolved_ceilings.is_empty() {
            return Err(self.unsupported(
                "invoked opaque callable has a contract ceiling but no retained entry origin",
            ));
        }
        Ok(())
    }
    fn parameter(
        &self,
        position: u32,
        site: InvocationSite,
        call: &crate::call_contracts::Invocation<'_>,
        reached: &mut Contributions,
    ) -> Checked<()> {
        let position = usize::try_from(position)
            .map_err(|_| self.foreign("invokes parameter coordinate is too large"))?;
        let order = call.contract.parameter_order;
        let kind = *order
            .get(position)
            .ok_or_else(|| self.foreign("invokes parameter escapes the public signature"))?;
        let lane = order[..position]
            .iter()
            .filter(|candidate| **candidate == kind)
            .count();
        let (contract, query) = match kind {
            BoundaryParameterKind::Scalar => {
                let Some(semantic_vocabulary::ScalarType::Callable(contract)) =
                    call.contract.scalar_parameters.scalar_type(lane)
                else {
                    return Err(self.foreign("invokes parameter names a non-callable actual"));
                };
                let actual = *call.inputs.arguments.get(lane).ok_or_else(|| {
                    self.foreign("invoked parameter lost its exact scalar actual")
                })?;
                (
                    contract,
                    CallableValueQuery::Scalar {
                        machine: self.caller.id,
                        value: actual,
                        contract,
                    },
                )
            }
            BoundaryParameterKind::Structural => {
                let formal = call
                    .contract
                    .structural_parameters
                    .get(lane)
                    .ok_or_else(|| {
                        self.foreign("invoked parameter lost its exact structural formal")
                    })?;
                let declaration = self
                    .module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == formal.structural_type)
                    .ok_or_else(|| {
                        self.foreign("invoked structural formal has no declared carrier")
                    })?;
                let terminal_psi::StructuralTypeShape::PrimitiveScalar(
                    semantic_vocabulary::ScalarType::Callable(contract),
                ) = declaration.shape
                else {
                    return Err(self.unsupported(
                        "invoked structural actual needs an exact nested callable parameter path",
                    ));
                };
                let values = self
                    .values
                    .ok_or_else(|| self.foreign("structural callable origins were not prepared"))?;
                let body = values
                    .bodies
                    .get(&self.caller.id)
                    .ok_or_else(|| self.foreign("structural callable actual lost its caller"))?;
                let key = CallableInvocationSite::Invocation(site);
                if let Some((_, reason)) = body.invalid_invocations.get(&key) {
                    return Err(self.foreign(reason));
                }
                let invocation = body.invocations.get(&key).ok_or_else(|| {
                    self.foreign("structural callable actual lost its exact invocation")
                })?;
                if invocation.scalar_arguments != call.inputs.arguments
                    || invocation.structural_actual_types.get(lane)
                        != Some(&(formal.structural_type, formal.access))
                {
                    return Err(self.foreign(
                        "structural callable actual differs from its retained invocation",
                    ));
                }
                let location = invocation
                    .structural_arguments
                    .get(lane)
                    .ok_or_else(|| {
                        self.foreign("invoked structural parameter lost its actual location")
                    })?
                    .clone();
                (
                    contract,
                    CallableValueQuery::Storage {
                        machine: self.caller.id,
                        position: invocation.position,
                        location,
                        contract,
                    },
                )
            }
        };
        let values = self.resolve(query)?;
        self.holder(contract, &values, reached)?;
        let envelope = &self.contract(contract)?.envelope;
        reached.operational(envelope);
        union(&mut reached.reach, envelope.service_reach.iter().copied());
        for entry in &envelope.invocations {
            match entry {
                Entry::Service(service) => union(&mut reached.entries, [Entry::Service(*service)]),
                Entry::Parameter(_) => {
                    return Err(self.unsupported(
                        "nested callable invocation needs its exact invocation actual mapping",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn judge(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<Vec<Excess>> {
    let module = &elaboration.module;
    let tables = &elaboration.tables;
    let Some(first) = module.machines.first().map(|machine| machine.id) else {
        return Ok(Vec::new());
    };
    terminal_semantics::callable_envelopes::validate_callable_operational_refinements(module)
        .map_err(|reason| fault(first, reason))?;
    for requirement in &elaboration.requirements {
        if elaboration.requirement_ceilings.get(*requirement).is_none()
            || module.machine(*requirement).is_none()
        {
            return Err(fault(
                first,
                "abstract requirement lost its exact source ceiling row",
            ));
        }
    }
    if elaboration.requirements.is_empty() && !elaboration.requirement_ceilings.is_empty() {
        return Err(fault(
            first,
            "concrete module contains abstract signature ceiling rows",
        ));
    }
    if !elaboration.requirements.is_empty()
        && (elaboration.requirement_ceilings.len() != module.machines.len()
            || elaboration
                .requirement_ceilings
                .keys()
                .any(|id| module.machine(id).is_none()))
    {
        return Err(fault(
            first,
            "abstract signature ceiling rows do not cover the exact named namespace",
        ));
    }
    for machine in &module.machines {
        let envelope = machine_envelope(elaboration, machine.id)?;
        if let Some(reach) = envelope.reach() {
            well_formed_reach(module, machine.id, reach)?;
        }
        if let Envelope::Runtime(envelope) = envelope {
            service_row(
                module,
                machine.id,
                &machine.published_synchronous_entries,
                false,
            )?;
            if envelope.service_reach != machine.published_service_ceiling
                || services(envelope) != machine.published_synchronous_entries
            {
                return Err(fault(
                    machine.id,
                    "named envelope differs from its published service projections",
                ));
            }
            if let Some(reach) = &machine.declared_service_reach {
                well_formed_reach(module, machine.id, reach)?;
                if *reach != envelope.service_reach {
                    return Err(fault(
                        machine.id,
                        "written service ceiling is not the published row",
                    ));
                }
            }
        } else if let Some(entries) = envelope.entries() {
            let services: Vec<_> = entries
                .into_iter()
                .filter_map(|entry| match entry {
                    Entry::Service(service) => Some(service),
                    Entry::Parameter(_) => None,
                })
                .collect();
            service_row(module, machine.id, &services, false)?;
        }
    }
    for boundary in &module.boundary_machines {
        let envelope = module
            .callables
            .boundary_envelope(boundary.id)
            .ok_or_else(|| fault(first, "boundary has no complete operational envelope"))?;
        well_formed_reach(module, first, &envelope.service_reach)?;
        service_row(
            module,
            first,
            &boundary.published_synchronous_entries,
            false,
        )?;
        if envelope.service_reach != boundary.published_service_ceiling
            || services(envelope) != boundary.published_synchronous_entries
        {
            return Err(fault(
                first,
                "boundary envelope differs from its published service projections",
            ));
        }
        well_formed_reach(module, first, &boundary.fixed_service_reach)?;
        if boundary
            .fixed_service_reach
            .iter()
            .any(|service| envelope.service_reach.binary_search(service).is_err())
        {
            return Err(fault(
                first,
                "boundary fixed reach is outside its published ceiling",
            ));
        }
    }
    for contract in &module.callables.contracts {
        well_formed_reach(module, first, &contract.envelope.service_reach)?;
    }
    if let Some(TargetMachineCallable::Machine(entry)) = module.entry
        && let Envelope::Runtime(envelope) = machine_envelope(elaboration, entry)?
        && envelope.service_reach != module.root_service_reach.concrete
    {
        return Err(fault(
            entry,
            "root service reach is not the entry's published row",
        ));
    }
    // Ordinary service-only calls need no storage/version reconstruction.
    // Code-value queries stay symbolic, preserving each formal's identity.
    let needs_values = tables.machines.values().any(|machine| {
        machine
            .invocations
            .values()
            .any(|call| matches!(call.callee, Callee::Selected { .. }))
    }) || module
        .callables
        .machine_envelopes
        .iter()
        .flatten()
        .any(|row| {
            row.envelope
                .invocations
                .iter()
                .any(|entry| matches!(entry, Entry::Parameter(_)))
        })
        || module
            .callables
            .boundary_envelopes
            .iter()
            .flatten()
            .any(|row| {
                row.envelope
                    .invocations
                    .iter()
                    .any(|entry| matches!(entry, Entry::Parameter(_)))
            });
    let values = needs_values
        .then(|| terminal_callable_values(module))
        .transpose()
        .map_err(|reason| fault(first, reason))?;
    let mut found = Vec::new();
    for caller in &module.machines {
        let rows = tables
            .machines
            .get(caller.id)
            .ok_or_else(|| fault(caller.id, "machine has no check tables"))?;
        let ceiling = machine_envelope(elaboration, caller.id)?;
        for (invocation, row) in rows.invocations.iter() {
            let at = match row.site {
                InvocationSite::Operation(operation) => Construct::Operation(operation),
                InvocationSite::ExternalCompletion(edge) | InvocationSite::InvokeNoNormal(edge) => {
                    Construct::Edge(edge)
                }
            };
            let call = crate::call_contracts::invocation(module, caller, rows, invocation, row)?;
            let envelope = envelope_for(elaboration, caller.id, call.inputs)?;
            if let Envelope::Runtime(envelope) = envelope {
                row.operational_acknowledgement
                    .require_envelope(envelope.suspends, envelope.blocks)
                    .map_err(|reason| refusal(RefusalReason::Unsupported, caller.id, at, reason))?;
                if envelope.suspends {
                    return Err(refusal(
                        RefusalReason::Unsupported,
                        caller.id,
                        at,
                        "suspending call needs continuation and carry evidence",
                    ));
                }
            }
            let context = Calls {
                module,
                values: values.as_ref(),
                caller,
                at,
            };
            let mut reached = Contributions::default();
            if let Some(reach) = envelope.reach() {
                reached.reach.extend_from_slice(reach);
            } else if ceiling.reach().is_some() {
                return Err(context.unsupported(
                    "open abstract service reach cannot establish a finite caller ceiling",
                ));
            }
            if let Some(entries) = envelope.entries() {
                for entry in entries {
                    match entry {
                        Entry::Service(service) => {
                            union(&mut reached.entries, [Entry::Service(service)])
                        }
                        Entry::Parameter(position) => {
                            context.parameter(position, row.site, &call, &mut reached)?
                        }
                    }
                }
            } else if ceiling.entries().is_some() {
                return Err(context.unsupported(
                    "open abstract invocation ceiling cannot establish a finite caller ceiling",
                ));
            }
            match envelope {
                Envelope::Runtime(envelope) => reached.operational(envelope),
                Envelope::Abstract(_) if matches!(ceiling, Envelope::Runtime(_)) => {
                    return Err(context.unsupported(
                        "abstract callable has no closed suspension, blocking or progress envelope",
                    ));
                }
                Envelope::Abstract(_) => {}
            }
            if let Callee::Selected { callable } = call.inputs.callee {
                let contract = call.inputs.contract.ok_or_else(|| {
                    context.foreign("selected invocation lost its exact contract")
                })?;
                let origins = context.resolve(CallableValueQuery::Scalar {
                    machine: caller.id,
                    value: callable,
                    contract,
                })?;
                context.holder(contract, &origins, &mut reached)?;
            }
            if reached.conditional_progress
                && matches!(ceiling, Envelope::Runtime(envelope) if matches!(envelope.termination, terminal_psi::TerminalCallableTermination::Terminates { .. }))
            {
                return Err(context
                    .unsupported("callee termination premises need their exact invocation proof"));
            }
            for axis in Axis::ALL {
                // None is an explicitly open abstract source permission, not
                // a missing runtime envelope or an inferred empty set.
                let Some(ceiling) = ceiling.row(axis) else {
                    continue;
                };
                let contributes = reached.row(axis);
                let outside: Vec<_> = contributes
                    .iter()
                    .copied()
                    .filter(|permission| ceiling.binary_search(permission).is_err())
                    .collect();
                if !outside.is_empty() {
                    found.push(Excess {
                        axis,
                        machine: caller.id,
                        at,
                        callee: call.inputs.callee,
                        ceiling,
                        contributes,
                        outside,
                    });
                }
            }
        }
    }
    Ok(found)
}
