//! Calls must fit the permissions their callers publish. Service reach and
//! synchronous entries share one owner, while retaining parameter identities.
//! A callable's type bounds what it may do; invoking its holder also enters
//! that exact formal, even when the type itself enters nothing further.

use semantic_vocabulary::{
    CallableContractId, InvocationSite, MachineId, ServiceId, TargetMachineCallable,
};
use std::borrow::Cow;
use terminal_check_tables::edges::Callee;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use terminal_psi::{
    BoundaryParameterKind, TerminalCallableEnvelope, TerminalCallableInvocation as Entry,
    TerminalMachine, TerminalModule,
};
use terminal_semantics::callable_envelopes::CallableOperationalView;
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
    /// An unknown bound stays unknown when its other contract axes are known.
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
            Self::Abstract(envelope) if envelope.envelope.is_some() => {
                Some(envelope.envelope.as_ref()?.invocations.clone())
            }
            Self::Abstract(envelope) => match &envelope.invokes {
                terminal_check_tables::contracts::Ceiling::Inferred => None,
                terminal_check_tables::contracts::Ceiling::Authored(row) => {
                    Some(row.iter().copied().map(Entry::Service).collect())
                }
            },
        }
    }
    fn suspension(self) -> Option<bool> {
        match self {
            Self::Runtime(envelope) => Some(envelope.suspends),
            Self::Abstract(envelope) => envelope.suspension,
        }
    }
    fn blocking(self) -> Option<bool> {
        match self {
            Self::Runtime(envelope) => Some(envelope.blocks),
            Self::Abstract(envelope) => envelope.blocking,
        }
    }
    fn termination(self) -> &'a terminal_psi::TerminalCallableTermination {
        match self {
            Self::Runtime(envelope) => &envelope.termination,
            Self::Abstract(envelope) => &envelope.termination,
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
            Axis::Suspension => self.suspension().map(|allowed| {
                allowed
                    .then_some(Permission::MaySuspend)
                    .into_iter()
                    .collect()
            }),
            Axis::Blocking => self.blocking().map(|allowed| {
                allowed
                    .then_some(Permission::MayBlock)
                    .into_iter()
                    .collect()
            }),
            Axis::Termination => Some(
                matches!(
                    self.termination(),
                    terminal_psi::TerminalCallableTermination::NotGuaranteed
                )
                .then_some(Permission::MayDiverge)
                .into_iter()
                .collect(),
            ),
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

fn fault(machine: MachineId, detail: &str) -> terminal_check_tables::Refusal {
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
    let runtime = elaboration.module.callables.machine_envelope(machine);
    if let Ok(position) = elaboration
        .check_form_envelopes
        .binary_search_by_key(&machine, |row| row.machine)
    {
        if runtime.is_some() {
            return Err(fault(machine, "check-form and runtime envelopes overlap"));
        }
        return Ok(Envelope::Runtime(
            &elaboration.check_form_envelopes[position].envelope,
        ));
    }
    if let Some(envelope) = runtime {
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
            if let Some(required) = inputs.required_contract {
                module
                    .invocation_machine_contract(machine, Some(required))
                    .ok_or_else(|| fault(caller, "selected call has no exact public contract"))?;
                module
                    .callables
                    .declaration_envelope(required)
                    .map(Envelope::Runtime)
                    .ok_or_else(|| {
                        fault(
                            caller,
                            "selected public contract has no complete operational envelope",
                        )
                    })
            } else {
                machine_envelope(elaboration, machine)
            }
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
    fn operational_from(
        &mut self,
        envelope: Envelope<'_>,
        ceiling: Envelope<'_>,
    ) -> Result<(), &'static str> {
        if (envelope.suspension().is_none() && ceiling.suspension().is_some())
            || (envelope.blocking().is_none() && ceiling.blocking().is_some())
        {
            return Err(
                "abstract callable has no closed suspension, blocking or progress envelope",
            );
        }
        self.operational(envelope);
        Ok(())
    }

    fn operational(&mut self, envelope: Envelope<'_>) {
        // Unknown source bounds contribute no asserted boolean. The caller
        // cannot use one to justify a known bound in operational_from.
        if let Some(suspends) = envelope.suspension() {
            self.suspends |= suspends;
        }
        if let Some(blocks) = envelope.blocking() {
            self.blocks |= blocks;
        }
        match envelope.termination() {
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
    ceiling: Envelope<'a>,
    axes: &'a [Axis],
    at: Construct,
}
impl Calls<'_> {
    fn operational(&self, envelope: Envelope<'_>, reached: &mut Contributions) -> Checked<()> {
        if self
            .axes
            .iter()
            .any(|axis| matches!(axis, Axis::Suspension | Axis::Blocking | Axis::Termination))
        {
            reached
                .operational_from(envelope, self.ceiling)
                .map_err(|reason| self.unsupported(reason))?;
        }
        Ok(())
    }
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
                    .body(self.caller.id)
                    .map_err(|reason| self.foreign(reason))?;
                let key = CallableInvocationSite::Invocation(site);
                let invocation = body
                    .invocation(key)
                    .map_err(|reason| self.foreign(reason))?;
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
        self.invoke_value(contract, query, reached)
    }

    fn tail_parameter(
        &self,
        position: u32,
        call: &crate::call_contracts::TailCall<'_>,
        reached: &mut Contributions,
    ) -> Checked<()> {
        let position = usize::try_from(position)
            .map_err(|_| self.foreign("tail invokes coordinate is too large"))?;
        let order = &call.callee.parameter_order;
        let kind = *order
            .get(position)
            .ok_or_else(|| self.foreign("tail invokes escapes its callable signature"))?;
        let lane = order[..position]
            .iter()
            .filter(|candidate| **candidate == kind)
            .count();
        let (contract, query) = match kind {
            BoundaryParameterKind::Scalar => {
                let Some(semantic_vocabulary::ScalarType::Callable(contract)) = call
                    .callee
                    .parameters
                    .get(lane)
                    .map(|formal| formal.scalar_type)
                else {
                    return Err(self.foreign("tail invokes names a non-callable formal"));
                };
                let value = *call
                    .arguments
                    .get(lane)
                    .ok_or_else(|| self.foreign("tail invokes lost its actual value"))?;
                (
                    contract,
                    CallableValueQuery::Scalar {
                        machine: self.caller.id,
                        value,
                        contract,
                    },
                )
            }
            BoundaryParameterKind::Structural => {
                let formal = call
                    .callee
                    .structural_parameters
                    .get(lane)
                    .ok_or_else(|| self.foreign("tail invokes lost its structural formal"))?;
                let declaration = self
                    .module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == formal.structural_type)
                    .ok_or_else(|| {
                        self.foreign("tail structural formal has no declared carrier")
                    })?;
                let terminal_psi::StructuralTypeShape::PrimitiveScalar(
                    semantic_vocabulary::ScalarType::Callable(contract),
                ) = declaration.shape
                else {
                    return Err(self.unsupported(
                        "invoked structural actual needs an exact nested callable parameter path",
                    ));
                };
                let actual = call
                    .structural_arguments
                    .get(lane)
                    .ok_or_else(|| self.foreign("tail invokes lost its structural actual"))?;
                let root = self
                    .caller
                    .structural_place_type(actual.place)
                    .ok_or_else(|| {
                        self.foreign("tail callable actual has no source declaration")
                    })?;
                let projected = terminal_semantics::runtime_structural_path_tip(
                    self.module.structural_types.iter(),
                    root,
                    &actual.path,
                )
                .ok_or_else(|| self.foreign("tail callable actual has no exact structural type"))?;
                if projected.id != formal.structural_type
                    || !terminal_psi::structural_access_covers(actual.access, formal.access)
                {
                    return Err(self.foreign("tail callable actual differs from its formal"));
                }
                let location = terminal_semantics::descriptor_lifetimes::descriptor_location(
                    &self.module.structural_types,
                    actual.place,
                    root,
                    &actual.path,
                    None,
                )
                .ok_or_else(|| {
                    self.foreign("tail callable actual has no exact descriptor coordinate")
                })?;
                let position = (
                    call.block.id,
                    u32::try_from(call.block.operations.len())
                        .map_err(|_| self.foreign("tail callable storage cut is too large"))?,
                );
                (
                    contract,
                    CallableValueQuery::Storage {
                        machine: self.caller.id,
                        position,
                        location,
                        contract,
                    },
                )
            }
        };
        self.invoke_value(contract, query, reached)
    }

    fn invoke_value(
        &self,
        contract: CallableContractId,
        query: CallableValueQuery,
        reached: &mut Contributions,
    ) -> Checked<()> {
        let values = self.resolve(query)?;
        self.holder(contract, &values, reached)?;
        let envelope = &self.contract(contract)?.envelope;
        self.operational(Envelope::Runtime(envelope), reached)?;
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

fn validate_check_form_envelopes(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<()> {
    let rows = &elaboration.check_form_envelopes;
    let Some(first) = rows.first() else {
        return Ok(());
    };
    if rows
        .windows(2)
        .any(|pair| pair[0].machine >= pair[1].machine)
    {
        return Err(fault(
            first.machine,
            "check-form envelopes repeat or disorder a machine identity",
        ));
    }
    let mut undeclared = elaboration
        .undeclared
        .iter()
        .map(|(machine, _)| *machine)
        .collect::<Vec<_>>();
    undeclared.sort_unstable();
    let module = &elaboration.module;
    for row in rows {
        let machine = module
            .machine(row.machine)
            .ok_or_else(|| fault(row.machine, "check-form envelope names an unknown machine"))?;
        if machine.declaration.is_some() || module.callables.machine_envelope(row.machine).is_some()
        {
            return Err(fault(
                row.machine,
                "check-form envelope overlaps a named runtime declaration",
            ));
        }
        if undeclared.binary_search(&row.machine).is_err() {
            return Err(fault(
                row.machine,
                "check-form envelope has no explicitly undeclared source frame",
            ));
        }
        if elaboration.requirements.contains(&row.machine) || machine.blocks.is_empty() {
            return Err(fault(
                row.machine,
                "check-form envelope names a bodyless requirement",
            ));
        }
        terminal_semantics::callable_envelopes::validate_callable_envelope(
            module,
            &row.envelope,
            &machine.parameter_order,
            &machine.parameters,
            &machine.structural_parameters,
        )
        .map_err(|reason| fault(row.machine, reason))?;
        if row.envelope.service_reach != machine.published_service_ceiling
            || services(&row.envelope) != machine.published_synchronous_entries
            || row.envelope.suspends != machine.published_may_suspend
            || row.envelope.blocks != machine.published_may_block
        {
            return Err(fault(
                row.machine,
                "check-form envelope differs from its complete published projections",
            ));
        }
    }
    Ok(())
}

fn validate_requirement_envelopes(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<()> {
    use terminal_check_tables::contracts::{Ceiling, StaticServiceCeiling};
    let module = &elaboration.module;
    let mut requirements = elaboration.requirements.clone();
    requirements.sort_unstable();
    let declarations = crate::static_contracts::requirement_declarations(elaboration)?;
    for (id, row) in elaboration.requirement_ceilings.iter() {
        let declaration = declarations.get(id).and_then(|source| *source);
        let Some(envelope) = &row.envelope else {
            if declaration.is_some() {
                return Err(fault(
                    id,
                    "abstract requirement lost its declared operational row",
                ));
            }
            continue;
        };
        let declaration = declaration.ok_or_else(|| {
            fault(
                id,
                "abstract requirement has no complete source declaration",
            )
        })?;
        if envelope != &declaration.envelope
            || row.suspension != Some(envelope.suspends)
            || row.blocking != Some(envelope.blocks)
            || row.termination != envelope.termination
        {
            return Err(fault(
                id,
                "abstract envelope differs from its complete source declaration",
            ));
        }
        let machine = module
            .machine(id)
            .ok_or_else(|| fault(id, "abstract envelope names no exact machine"))?;
        if requirements.binary_search(&id).is_err()
            || !machine.blocks.is_empty()
            || machine.declaration.is_some()
            || module.callables.machine_envelope(id).is_some()
        {
            return Err(fault(
                id,
                "abstract envelope does not belong to a bodyless requirement",
            ));
        }
        let reach = match (&row.reaches, &envelope.service_reach) {
            (Ceiling::Inferred, StaticServiceCeiling::Open) => None,
            (Ceiling::Authored(expected), StaticServiceCeiling::Closed(actual))
                if expected == actual =>
            {
                Some(actual.as_slice())
            }
            _ => {
                return Err(fault(
                    id,
                    "abstract envelope changes its declared service ceiling",
                ));
            }
        };
        let entries = envelope
            .invocations
            .iter()
            .filter_map(|entry| match entry {
                Entry::Service(service) => Some(*service),
                Entry::Parameter(_) => None,
            })
            .collect::<Vec<_>>();
        if !matches!(&row.invokes, Ceiling::Authored(expected) if expected == &entries)
            || reach.is_some_and(|reach| reach != machine.published_service_ceiling)
            || entries != machine.published_synchronous_entries
            || envelope.suspends != machine.published_may_suspend
            || envelope.blocks != machine.published_may_block
            || (machine.contract.termination == semantic_vocabulary::Termination::Terminates)
                != matches!(
                    envelope.termination,
                    terminal_psi::TerminalCallableTermination::Terminates { .. }
                )
        {
            return Err(fault(
                id,
                "abstract envelope differs from its exact published header",
            ));
        }
        terminal_semantics::callable_envelopes::validate_callable_bounds(
            module,
            reach,
            CallableOperationalView {
                invocations: &envelope.invocations,
                suspends: envelope.suspends,
                blocks: envelope.blocks,
                termination: &envelope.termination,
            },
            &machine.parameter_order,
            &machine.parameters,
            &machine.structural_parameters,
        )
        .map_err(|reason| fault(id, reason))?;
    }
    Ok(())
}

pub(crate) fn judge(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<Vec<Excess>> {
    judge_axes(elaboration, &Axis::ALL)
}

/// A logical refinement proves the declared service and entry rows without
/// claiming executable acceptance. Its caller still owes operational refinement
/// at descriptor admission, where every axis is checked independently.
pub(crate) fn judge_declared_rows(
    elaboration: &terminal_check_tables::program::Elaboration,
) -> Checked<Vec<Excess>> {
    judge_axes(elaboration, &[Axis::Reach, Axis::Entries])
}

fn judge_axes(
    elaboration: &terminal_check_tables::program::Elaboration,
    axes: &[Axis],
) -> Checked<Vec<Excess>> {
    let module = &elaboration.module;
    let tables = &elaboration.tables;
    if elaboration
        .installation_bound_refinement
        .is_some_and(|root| module.entry != Some(TargetMachineCallable::Machine(root)))
    {
        return Err(fault(
            elaboration
                .installation_bound_refinement
                .expect("present refinement root"),
            "installation reach refinement does not name its proof root",
        ));
    }
    validate_check_form_envelopes(elaboration)?;
    validate_requirement_envelopes(elaboration)?;
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
    for row in &module.callables.declaration_envelopes {
        well_formed_reach(module, first, &row.envelope.service_reach)?;
    }
    if let Some(TargetMachineCallable::Machine(entry)) = module.entry
        && let Envelope::Runtime(envelope) = machine_envelope(elaboration, entry)?
    {
        // An unresolved installation row is bounded by its requirement, not
        // an unconditional contribution to this root's concrete services.
        let root_ceiling = if module
            .root_service_reach
            .installation_dependencies
            .is_empty()
        {
            Cow::Borrowed(module.root_service_reach.concrete.as_slice())
        } else {
            let mut ceiling = module.root_service_reach.concrete.clone();
            for dependency in &module.root_service_reach.installation_dependencies {
                ceiling.extend(dependency.upper_bound.iter().copied());
            }
            ceiling.sort_unstable();
            ceiling.dedup();
            Cow::Owned(ceiling)
        };
        if envelope.service_reach.as_slice() != root_ceiling.as_ref() {
            return Err(fault(
                entry,
                "root service reach is not the entry's published row",
            ));
        }
    }
    // Ordinary service-only calls need no storage/version reconstruction.
    // Code-value queries stay symbolic, preserving each formal's identity.
    let needs_values = tables.machines.values().any(|machine| {
        machine
            .invocations
            .values()
            .any(|call| matches!(call.callee, Callee::Selected { .. }))
    }) || elaboration.requirement_ceilings.values().any(|row| {
        row.envelope.as_ref().is_some_and(|envelope| {
            envelope
                .invocations
                .iter()
                .any(|entry| matches!(entry, Entry::Parameter(_)))
        })
    }) || elaboration.check_form_envelopes.iter().any(|row| {
        row.envelope
            .invocations
            .iter()
            .any(|entry| matches!(entry, Entry::Parameter(_)))
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
    let needs_values = needs_values
        || module.callables.declaration_envelopes.iter().any(|row| {
            row.envelope
                .invocations
                .iter()
                .any(|entry| matches!(entry, Entry::Parameter(_)))
        });
    let values = needs_values
        .then(|| terminal_callable_values(module))
        .transpose()
        .map_err(|reason| fault(first, &reason))?;
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
                InvocationSite::ExternalCompletion(edge)
                | InvocationSite::InvokeNoNormal(edge)
                | InvocationSite::Cleanup { edge, .. } => Construct::Edge(edge),
            };
            let call = crate::call_contracts::invocation(module, caller, rows, invocation, row)?;
            let envelope = envelope_for(elaboration, caller.id, call.inputs)?;
            let context = Calls {
                module,
                values: values.as_ref(),
                caller,
                ceiling,
                axes,
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
            context.operational(envelope, &mut reached)?;
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
                && matches!(
                    ceiling.termination(),
                    terminal_psi::TerminalCallableTermination::Terminates { .. }
                )
            {
                return Err(context
                    .unsupported("callee termination premises need their exact invocation proof"));
            }
            if elaboration.installation_bound_refinement == Some(caller.id)
                && let (Some(ceiling), Callee::Declared(callee)) =
                    (ceiling.reach(), call.inputs.callee)
                && reached
                    .reach
                    .iter()
                    .any(|service| ceiling.binary_search(service).is_err())
            {
                reached.reach = fixed_refinement_reach(
                    module,
                    tables,
                    caller.id,
                    callee,
                    call.inputs.required_contract,
                )?;
            }
            contain(
                ceiling,
                &reached,
                caller.id,
                call.inputs.callee,
                at,
                &mut found,
                axes,
            );
        }
        let tails = crate::call_contracts::TailCallSites::new(caller);
        for formation in &caller.call_formations {
            if let (terminal_psi::CallSite::Edge(edge), Some(callee)) =
                (formation.site, formation.tail_callee)
            {
                tail_contribution(
                    elaboration,
                    Calls {
                        module,
                        values: values.as_ref(),
                        caller,
                        ceiling,
                        axes,
                        at: Construct::Edge(edge),
                    },
                    &tails,
                    callee,
                    edge,
                    &mut found,
                )?;
            }
        }
    }
    Ok(found)
}

fn contain(
    ceiling: Envelope<'_>,
    reached: &Contributions,
    machine: MachineId,
    callee: Callee,
    at: Construct,
    found: &mut Vec<Excess>,
    axes: &[Axis],
) {
    for &axis in axes {
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
                machine,
                at,
                callee,
                ceiling,
                contributes,
                outside,
            });
        }
    }
}

/// A frame-reusing jump retains its exact nominal interface. It is not an
/// invocation row: substitute the jump's actual scalar values directly.
fn tail_contribution(
    elaboration: &terminal_check_tables::program::Elaboration,
    context: Calls<'_>,
    tails: &crate::call_contracts::TailCallSites<'_>,
    callee: MachineId,
    edge: semantic_vocabulary::EdgeId,
    found: &mut Vec<Excess>,
) -> Checked<()> {
    let module = &elaboration.module;
    let caller = context.caller;
    let at = context.at;
    let ceiling = context.ceiling;
    let tail = crate::call_contracts::tail_call(module, tails, callee, edge)?;
    let envelope = machine_envelope(elaboration, callee)?;
    let mut reached = Contributions::default();
    if let Some(reach) = envelope.reach() {
        reached.reach.extend_from_slice(reach);
    } else if ceiling.reach().is_some() {
        return Err(context
            .unsupported("open abstract service reach cannot establish a finite caller ceiling"));
    }
    if let Some(entries) = envelope.entries() {
        for entry in entries {
            match entry {
                Entry::Service(service) => union(&mut reached.entries, [Entry::Service(service)]),
                Entry::Parameter(position) => {
                    context.tail_parameter(position, &tail, &mut reached)?;
                }
            }
        }
    } else if ceiling.entries().is_some() {
        return Err(context.unsupported(
            "open abstract invocation ceiling cannot establish a finite caller ceiling",
        ));
    }
    context.operational(envelope, &mut reached)?;
    if reached.conditional_progress
        && matches!(
            ceiling.termination(),
            terminal_psi::TerminalCallableTermination::Terminates { .. }
        )
    {
        return Err(
            context.unsupported("callee termination premises need their exact invocation proof")
        );
    }
    if elaboration.installation_bound_refinement == Some(caller.id)
        && let Some(ceiling) = ceiling.reach()
        && reached
            .reach
            .iter()
            .any(|service| ceiling.binary_search(service).is_err())
    {
        reached.reach =
            fixed_refinement_reach(module, &elaboration.tables, caller.id, callee.into(), None)?;
    }
    contain(
        ceiling,
        &reached,
        caller.id,
        Callee::Declared(callee.into()),
        at,
        found,
        context.axes,
    );
    Ok(())
}

/// Only the proof of an abstract installation requirement may leave its child
/// rows conditional. Explicit machine headers and ordinary boundary headers
/// remain pinned; inferred helpers contribute their known calls and services.
fn fixed_refinement_reach(
    module: &TerminalModule,
    tables: &terminal_check_tables::CheckTables,
    from: MachineId,
    callee: TargetMachineCallable,
    required_contract: Option<semantic_vocabulary::ContractId>,
) -> Checked<Vec<ServiceId>> {
    let mut pending = vec![(callee, required_contract)];
    let mut visited = tables.machines.map(|_, _| false);
    let mut fixed = Vec::new();
    while let Some((callee, required_contract)) = pending.pop() {
        match callee {
            TargetMachineCallable::Boundary(id) => {
                let boundary = module
                    .boundary_machine(id)
                    .ok_or_else(|| fault(from, "refinement lost a boundary declaration"))?;
                if !boundary.service_reach_is_installation_bound {
                    fixed.extend(boundary.published_service_ceiling.iter().copied());
                }
            }
            TargetMachineCallable::Machine(id) => {
                let contract = module
                    .invocation_machine_contract(id, required_contract)
                    .ok_or_else(|| fault(from, "refinement lost a machine contract"))?;
                if required_contract.is_some() || contract.declared_service_reach.is_some() {
                    fixed.extend(contract.published_service_ceiling.iter().copied());
                    continue;
                }
                let seen = visited
                    .get_mut(id)
                    .ok_or_else(|| fault(from, "refinement machine has no checking rows"))?;
                if std::mem::replace(seen, true) {
                    continue;
                }
                let body = crate::terminal_machine(&module.machines, id, from)?;
                if body.blocks.is_empty() {
                    return Err(fault(from, "inferred reach has no executable body"));
                }
                let rows = tables
                    .machines
                    .get(id)
                    .ok_or_else(|| fault(from, "refinement lost a machine's checking rows"))?;
                // Generated owned cleanup uses these same exact invocation rows,
                // including the selected consumer and its required contract.
                for (_, call) in rows.invocations.iter() {
                    match call.callee {
                        Callee::Declared(callee) => pending.push((callee, call.required_contract)),
                        Callee::Selected { .. } => {
                            return Err(refusal(
                                RefusalReason::Unsupported,
                                from,
                                Construct::Machine,
                                "installation refinement reach through a selector value",
                            ));
                        }
                    }
                }
                for formation in &body.call_formations {
                    if let (terminal_psi::CallSite::Edge(_), Some(callee)) =
                        (formation.site, formation.tail_callee)
                    {
                        pending.push((callee.into(), None));
                    }
                }
                for operation in body.operations() {
                    let service = match &operation.kind {
                        terminal_psi::OperationKind::PortWrite { service, .. } => Some(*service),
                        terminal_psi::OperationKind::RegisterAccess { service, .. } => *service,
                        _ => None,
                    };
                    if let Some(service) = service {
                        let declaration = module
                            .service(service)
                            .ok_or_else(|| fault(from, "refinement lost an instruction service"))?;
                        fixed.push(service);
                        fixed.extend(declaration.parents.iter().copied());
                    }
                }
            }
        }
    }
    fixed.sort_unstable();
    fixed.dedup();
    Ok(fixed)
}
