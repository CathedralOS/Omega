//! Keep each callable's authored permissions separate from what its body reaches.
//! Private omitted axes are inferred from actual calls, never from callers or a
//! list of eligible implementations. Published axes are checked as ceilings.

use crate::reach::{SourceEnvelope, SourceInvocation as Invocation};
use language_semantics::{
    ServiceReachId, ServiceReachRowId, TerminationGuarantee, TerminationInterface,
};
use semantic_vocabulary::{IdTable, MachineId};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::TerminalCallableTermination;
use typed_trees::TypedTrees;
use typed_trees::signature::{AuthoredInvocation, AuthoredInvocationTarget, StateSignature};

use super::Program;
use crate::body::{
    Body, CallTarget, Expression, ExpressionKind as E, Statement, Target, ValueType,
};

pub(crate) fn unavailable(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        machine,
        concerns: Construct::Machine,
        reason: RefusalReason::Unsupported,
        detail: detail.into(),
    }
}

pub(super) fn service(
    typed: &TypedTrees,
    context: MachineId,
    id: ServiceReachId,
) -> Checked<ServiceReachId> {
    typed
        .service_reaches
        .definition(id)
        .ok_or_else(|| unavailable(context, "service permission names an absent declaration"))?;
    Ok(id)
}

pub(super) fn service_closure(
    typed: &TypedTrees,
    context: MachineId,
    roots: &[ServiceReachId],
) -> Checked<Vec<ServiceReachId>> {
    crate::reach::source_closure(typed, context, roots)
}

pub(crate) fn finite_reach(
    typed: &TypedTrees,
    context: MachineId,
    mut row: ServiceReachRowId,
) -> Checked<Vec<ServiceReachId>> {
    let mut active = Vec::new();
    loop {
        if row.0 == 0 || row.0 as usize > typed.service_reach_rows.len() || active.contains(&row) {
            return Err(unavailable(
                context,
                "callable has an open, stale or cyclic service ceiling",
            ));
        }
        active.push(row);
        match typed.service_reach_rows.abstract_bound(row) {
            Some(bound) => row = bound,
            None => return service_closure(typed, context, typed.service_reach_rows.services(row)),
        }
    }
}

pub(super) fn invocations(
    typed: &TypedTrees,
    context: MachineId,
    authored: &[AuthoredInvocation],
    parameters: &[typed_trees::signature::StateParameter],
) -> Checked<Vec<Invocation>> {
    let mut targets = Vec::new();
    for invocation in authored {
        let target = match invocation.target {
            AuthoredInvocationTarget::Parameter { ordinal, symbol }
                if symbol.is_valid()
                    && parameters
                        .get(ordinal as usize)
                        .is_some_and(|parameter| parameter.symbol == symbol) =>
            {
                let source = ordinal as usize;
                if parameters[source].relevance != language_core::BindingRelevance::Relevant {
                    return Err(unavailable(
                        context,
                        "invocation permission names an erased formal",
                    ));
                }
                let runtime = parameters[..source]
                    .iter()
                    .filter(|formal| formal.relevance == language_core::BindingRelevance::Relevant)
                    .count();
                Invocation::Parameter(u32::try_from(runtime).map_err(|_| {
                    unavailable(context, "invocation runtime parameter coordinate overflow")
                })?)
            }
            AuthoredInvocationTarget::Service(symbol) if symbol.is_valid() => {
                let id = typed.service_reaches.id_for_symbol(symbol).ok_or_else(|| {
                    unavailable(
                        context,
                        "invocation permission has no exact service declaration",
                    )
                })?;
                Invocation::Service(service(typed, context, id)?)
            }
            _ => {
                return Err(unavailable(
                    context,
                    "invocation permission has an unresolved target",
                ));
            }
        };
        targets.push(target);
    }
    targets.sort();
    targets.dedup();
    Ok(targets)
}

pub(super) fn termination(
    context: MachineId,
    guarantee: &TerminationGuarantee,
) -> Checked<TerminalCallableTermination> {
    match guarantee {
        TerminationGuarantee::NoGuarantee => Ok(TerminalCallableTermination::NotGuaranteed),
        TerminationGuarantee::Terminates { premises } if premises.is_empty() => {
            Ok(TerminalCallableTermination::Terminates {
                premises: Vec::new(),
            })
        }
        TerminationGuarantee::Terminates { .. } => Err(unavailable(
            context,
            "callable termination premises need retained lowered progress subjects",
        )),
    }
}

impl Program<'_> {
    pub(super) fn signature_envelope(
        &self,
        context: MachineId,
        signature: &StateSignature,
    ) -> Checked<SourceEnvelope> {
        if signature.service_reach_is_installation_bound {
            return Err(unavailable(
                context,
                "callable has an unsettled installation service ceiling",
            ));
        }
        let authored = self
            .typed
            .signature_invokes
            .span(signature.invokes)
            .ok_or_else(|| unavailable(context, "callable invocation permission span is stale"))?;
        let parameters = self
            .typed
            .state_parameters
            .span(signature.parameters)
            .ok_or_else(|| unavailable(context, "callable parameter permission span is stale"))?;
        Ok(SourceEnvelope {
            service_reach: finite_reach(self.typed, context, signature.service_reach_row)?,
            invocations: invocations(self.typed, context, authored, parameters)?,
            suspends: signature.suspends,
            blocks: signature.blocks,
            termination: termination(context, &signature.termination_guarantee)?,
        })
    }

    pub(crate) fn finish_operational_envelopes(
        &mut self,
        bodies: &IdTable<MachineId, Body>,
    ) -> Checked<()> {
        // An abstract requirement can defer its operational contract to a
        // later application. The generic witness retains that open Signature;
        // it cannot publish a complete callable envelope or supply descriptors.
        if let Some(requirement) = self.requirements.iter().copied().find(|id| {
            self.symbols
                .get(*id)
                .is_some_and(|symbol| self.typed.machine_by_symbol(*symbol).is_none())
                && !matches!(self.callables.envelopes.get(*id), Some(Some(_)))
        }) {
            if !self.callables.contracts.is_empty() || !self.callables.descriptors.is_empty() {
                return Err(unavailable(
                    requirement,
                    "runtime callable needs the abstract requirement's complete operational contract",
                ));
            }
            return Ok(());
        }
        let mut policies = IdTable::default();
        let mut published = IdTable::default();
        let mut calls = IdTable::default();
        for (id, symbol) in self.symbols.iter() {
            let policy = if let Some(machine) = self.typed.machine_by_symbol(*symbol) {
                if machine.service_reach_is_installation_bound {
                    return Err(unavailable(
                        id,
                        "named callable has an unsettled installation service ceiling",
                    ));
                }
                let authored = self
                    .typed
                    .signature_invokes
                    .span(machine.invokes)
                    .ok_or_else(|| unavailable(id, "named invocation permission span is stale"))?;
                let states = self
                    .typed
                    .machine_states
                    .span(machine.states)
                    .ok_or_else(|| unavailable(id, "named callable has a stale state span"))?;
                let [state] = states else {
                    return Err(unavailable(
                        id,
                        "named callable needs one exact entry state",
                    ));
                };
                let parameters = self
                    .typed
                    .state_parameters
                    .span(state.parameters)
                    .ok_or_else(|| unavailable(id, "named callable has a stale parameter span"))?;
                let termination = match &machine.termination_plan.interface {
                    TerminationInterface::Published(guarantee) => termination(id, guarantee)?,
                    TerminationInterface::InternalDerived => {
                        TerminalCallableTermination::NotGuaranteed
                    }
                };
                Policy {
                    reach: crate::reach::written_ceiling(self.typed, id, *symbol)?,
                    invokes: if !authored.is_empty() {
                        Some(invocations(self.typed, id, authored, parameters)?)
                    } else {
                        None
                    },
                    suspends: (machine.is_public
                        || machine.suspends
                        || !machine.suspends_keyword_source_spans.is_empty())
                    .then_some(machine.suspends),
                    blocks: (machine.is_public
                        || machine.blocks
                        || !machine.blocks_keyword_source_spans.is_empty())
                    .then_some(machine.blocks),
                    termination,
                }
            } else {
                let envelope = self
                    .callables
                    .envelopes
                    .get(id)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| {
                        unavailable(id, "abstract callable has no retained operational contract")
                    })?;
                Policy {
                    reach: Some(envelope.service_reach.clone()),
                    invokes: Some(envelope.invocations.clone()),
                    suspends: Some(envelope.suspends),
                    blocks: Some(envelope.blocks),
                    termination: envelope.termination.clone(),
                }
            };
            published.push(policy.initial());
            policies.push(policy);
            let body = terminal_check_tables::row(bodies, id, id, Construct::Machine)?;
            let mut found = Vec::new();
            for statement in &body.statements {
                collect_statement(statement, &mut found);
            }
            calls.push(found);
        }
        loop {
            let mut changed = false;
            for (id, uses) in calls.iter() {
                let mut reached = empty();
                for invocation in uses {
                    let target_signature = self.source_call_signature(id, invocation.target)?;
                    let envelope = match invocation.target {
                        CallTarget::Machine(callee) => {
                            terminal_check_tables::row(&published, *callee, id, Construct::Machine)?
                        }
                        CallTarget::Selected { contract, .. } => {
                            &self
                                .callables
                                .contracts
                                .get(*contract)
                                .ok_or_else(|| {
                                    unavailable(
                                        id,
                                        "invocation lost its retained callable contract",
                                    )
                                })?
                                .envelope
                        }
                        CallTarget::Boundary { boundary, .. } => {
                            &self
                                .boundaries
                                .row(*boundary)
                                .ok_or_else(|| {
                                    unavailable(
                                        id,
                                        "invocation lost its retained boundary contract",
                                    )
                                })?
                                .envelope
                        }
                    };
                    union_services(&mut reached.service_reach, &envelope.service_reach);
                    reached.suspends |= envelope.suspends;
                    reached.blocks |= envelope.blocks;
                    for target in &envelope.invocations {
                        match target {
                            Invocation::Service(service) => {
                                union(&mut reached.invocations, &[Invocation::Service(*service)])
                            }
                            Invocation::Parameter(position) => {
                                let source_position = target_signature
                                    .source_parameter_position(*position as usize)
                                    .ok_or_else(|| {
                                        unavailable(
                                            id,
                                            "invocation permission names no runtime formal",
                                        )
                                    })?;
                                let actual =
                                    invocation.arguments.get(source_position).ok_or_else(|| {
                                        unavailable(
                                            id,
                                            "invocation permission lost its exact actual",
                                        )
                                    })?;
                                match &actual.kind {
                                    E::Slot(slot)
                                        if *slot
                                            < self
                                                .signatures
                                                .get(id)
                                                .ok_or_else(|| {
                                                    unavailable(id, "caller has no signature")
                                                })?
                                                .parameters
                                                .len() =>
                                    {
                                        union(
                                            &mut reached.invocations,
                                            &[Invocation::Parameter(
                                                u32::try_from(self.signatures.get(id)
                                                    .and_then(|signature| signature.runtime_parameter_position(*slot))
                                                    .ok_or_else(|| unavailable(id, "invocation actual names no runtime caller formal"))?).map_err(|_| {
                                                    unavailable(id, "invocation parameter coordinate overflow")
                                                })?,
                                            )],
                                        )
                                    }
                                    _ => {
                                        let Some(ValueType::Scalar(
                                            semantic_vocabulary::ScalarType::Callable(contract),
                                            _,
                                        )) = actual.result
                                        else {
                                            return Err(unavailable(
                                                id,
                                                "invocation permission actual has no retained callable type",
                                            ));
                                        };
                                        let actual = self.callables.contracts.get(contract).ok_or_else(|| unavailable(id, "invoked actual has no retained callable contract"))?;
                                        union_services(
                                            &mut reached.service_reach,
                                            &actual.envelope.service_reach,
                                        );
                                        reached.suspends |= actual.envelope.suspends;
                                        reached.blocks |= actual.envelope.blocks;
                                        for dependency in &actual.envelope.invocations {
                                            match dependency {
                                                Invocation::Service(service) => union(
                                                    &mut reached.invocations,
                                                    &[Invocation::Service(*service)],
                                                ),
                                                Invocation::Parameter(_) => {
                                                    return Err(unavailable(
                                                        id,
                                                        "nested invocation permission needs the invocation's exact actual mapping",
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let CallTarget::Selected { callable, .. } = invocation.target
                        && let E::Slot(slot) = callable.kind
                        && slot
                            < self
                                .signatures
                                .get(id)
                                .ok_or_else(|| unavailable(id, "caller has no signature"))?
                                .parameters
                                .len()
                    {
                        union(
                            &mut reached.invocations,
                            &[Invocation::Parameter(
                                u32::try_from(
                                    self.signatures
                                        .get(id)
                                        .and_then(|signature| {
                                            signature.runtime_parameter_position(slot)
                                        })
                                        .ok_or_else(|| {
                                            unavailable(
                                                id,
                                                "selected callee names no runtime caller formal",
                                            )
                                        })?,
                                )
                                .map_err(|_| {
                                    unavailable(id, "invocation parameter coordinate overflow")
                                })?,
                            )],
                        );
                    }
                }
                let next = policies
                    .get(id)
                    .ok_or_else(|| unavailable(id, "callable has no header policy"))?
                    .close(id, reached)?;
                let previous = published
                    .get_mut(id)
                    .ok_or_else(|| unavailable(id, "callable has no envelope row"))?;
                if *previous != next {
                    *previous = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // A silent private acyclic body may finish locally without exporting
        // a promise. Unknown code must still carry a genuine termination
        // guarantee before a caller can rely on it to fulfill its own promise.
        let mut local_termination = policies.map(|_, policy| {
            matches!(
                policy.termination,
                TerminalCallableTermination::Terminates { .. }
            )
        });
        loop {
            let mut changed = false;
            for (id, uses) in calls.iter() {
                if self.requirements.contains(&id) || local_termination.get(id) == Some(&true) {
                    continue;
                }
                let symbol = self
                    .symbols
                    .get(id)
                    .ok_or_else(|| unavailable(id, "termination subject has no exact source"))?;
                let private = self
                    .typed
                    .machine_by_symbol(*symbol)
                    .is_some_and(|machine| !machine.is_public);
                if private
                    && uses.iter().all(|invocation| {
                        call_terminates(self, invocation.target, &local_termination)
                    })
                {
                    *local_termination
                        .get_mut(id)
                        .ok_or_else(|| unavailable(id, "termination subject has no row"))? = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (id, envelope) in published.iter() {
            if matches!(
                envelope.termination,
                TerminalCallableTermination::Terminates { .. }
            ) && calls
                .get(id)
                .ok_or_else(|| unavailable(id, "termination subject has no invocations"))?
                .iter()
                .any(|invocation| !call_terminates(self, invocation.target, &local_termination))
            {
                return Err(unavailable(
                    id,
                    "termination promise invokes a callable without a termination guarantee",
                ));
            }
            self.callables
                .bind_machine_envelope(id, envelope.clone())
                .map_err(|reason| unavailable(id, reason))?;
        }
        Ok(())
    }
}

fn call_terminates(
    program: &Program<'_>,
    target: &CallTarget,
    named: &IdTable<MachineId, bool>,
) -> bool {
    match target {
        CallTarget::Machine(machine) => named.get(*machine) == Some(&true),
        CallTarget::Selected { contract, .. } => program
            .callables
            .contracts
            .get(*contract)
            .is_some_and(|row| {
                matches!(
                    row.envelope.termination,
                    TerminalCallableTermination::Terminates { .. }
                )
            }),
        CallTarget::Boundary { boundary, .. } => {
            program.boundaries.row(*boundary).is_some_and(|row| {
                matches!(
                    row.envelope.termination,
                    TerminalCallableTermination::Terminates { .. }
                )
            })
        }
    }
}

struct Policy {
    reach: Option<Vec<ServiceReachId>>,
    invokes: Option<Vec<Invocation>>,
    suspends: Option<bool>,
    blocks: Option<bool>,
    termination: TerminalCallableTermination,
}
impl Policy {
    fn initial(&self) -> SourceEnvelope {
        SourceEnvelope {
            service_reach: self.reach.clone().unwrap_or_default(),
            invocations: self.invokes.clone().unwrap_or_default(),
            suspends: self.suspends.unwrap_or(false),
            blocks: self.blocks.unwrap_or(false),
            termination: self.termination.clone(),
        }
    }
    fn close(&self, context: MachineId, reached: SourceEnvelope) -> Checked<SourceEnvelope> {
        if self.invokes.as_ref().is_some_and(|ceiling| {
            reached
                .invocations
                .iter()
                .any(|value| !ceiling.contains(value))
        }) {
            return Err(unavailable(
                context,
                "body invokes outside its published ceiling",
            ));
        }
        if self.suspends == Some(false) && reached.suspends {
            return Err(unavailable(
                context,
                "body suspends outside its published ceiling",
            ));
        }
        if self.blocks == Some(false) && reached.blocks {
            return Err(unavailable(
                context,
                "body blocks outside its published ceiling",
            ));
        }
        Ok(SourceEnvelope {
            service_reach: self.reach.clone().unwrap_or(reached.service_reach),
            invocations: self.invokes.clone().unwrap_or(reached.invocations),
            suspends: self.suspends.unwrap_or(reached.suspends),
            blocks: self.blocks.unwrap_or(reached.blocks),
            termination: self.termination.clone(),
        })
    }
}
fn empty() -> SourceEnvelope {
    SourceEnvelope {
        service_reach: Vec::new(),
        invocations: Vec::new(),
        suspends: false,
        blocks: false,
        termination: TerminalCallableTermination::NotGuaranteed,
    }
}
fn union_services(to: &mut Vec<ServiceReachId>, from: &[ServiceReachId]) {
    to.extend_from_slice(from);
    to.sort_by_key(|service| service.0);
    to.dedup();
}
fn union<T: Clone + Ord>(to: &mut Vec<T>, from: &[T]) {
    to.extend_from_slice(from);
    to.sort();
    to.dedup();
}
struct Use<'a> {
    target: &'a CallTarget,
    arguments: &'a [Expression],
}
fn collect_statement<'a>(statement: &'a Statement, calls: &mut Vec<Use<'a>>) {
    match statement {
        Statement::Bind { value, .. } | Statement::Evaluate(value) => {
            collect_expression(value, calls)
        }
        Statement::Store {
            destination, value, ..
        } => {
            collect_expression(destination, calls);
            collect_expression(value, calls);
        }
        Statement::Witness { .. } | Statement::Declare { .. } => {}
        Statement::Dispatch {
            subject,
            arms,
            default,
            ..
        } => {
            collect_expression(subject, calls);
            for (_, target, _) in arms {
                collect_target(target, calls);
            }
            if let Some((target, _)) = default {
                collect_target(target, calls);
            }
        }
        Statement::Arm { guard, target, .. } => {
            if let Some(guard) = guard {
                collect_expression(guard, calls);
            }
            collect_target(target, calls);
        }
    }
}
fn collect_target<'a>(target: &'a Target, calls: &mut Vec<Use<'a>>) {
    if let Target::Value(value) = target {
        collect_expression(value, calls);
    }
}
fn collect_expression<'a>(expression: &'a Expression, calls: &mut Vec<Use<'a>>) {
    match &expression.kind {
        E::Call {
            callee, arguments, ..
        } => {
            if let CallTarget::Selected { callable, .. } = callee {
                collect_expression(callable, calls);
            }
            for argument in arguments {
                collect_expression(argument, calls);
            }
            calls.push(Use {
                target: callee,
                arguments,
            });
        }
        E::Field { base, .. }
        | E::Payload { base, .. }
        | E::IsCase { subject: base, .. }
        | E::Borrow { place: base, .. }
        | E::Not(base)
        | E::BitwiseNot(base)
        | E::Cast(base)
        | E::Requalify(base) => collect_expression(base, calls),
        E::Record { fields } | E::Case { fields, .. } => {
            for (_, field) in fields {
                collect_expression(field, calls);
            }
        }
        E::Match {
            subject,
            arms,
            default,
            ..
        } => {
            collect_expression(subject, calls);
            for (_, arm) in arms {
                collect_expression(arm, calls);
            }
            if let Some(default) = default {
                collect_expression(default, calls);
            }
        }
        E::Strict { left, right, .. } | E::ShortCircuit { left, right, .. } => {
            collect_expression(left, calls);
            collect_expression(right, calls);
        }
        E::Array { elements } => {
            for element in elements {
                collect_expression(element, calls);
            }
        }
        E::Index { base, index } => {
            collect_expression(base, calls);
            collect_expression(index, calls);
        }
        E::SavedSubject(_)
        | E::ByteLiteral(_)
        | E::Integer(_)
        | E::Boolean(_)
        | E::Slot(_)
        | E::MachineValue { .. } => {}
    }
}

impl Program<'_> {
    pub(crate) fn service_map(&mut self, context: MachineId) -> Checked<crate::reach::ServiceMap> {
        let mut roots = Vec::new();
        for (_, row) in self.callables.contracts.iter() {
            source_roots(&row.envelope, &mut roots);
        }
        for (_, row) in self.callables.envelopes.iter() {
            if let Some(row) = row {
                source_roots(row, &mut roots);
            }
        }
        for (_, row) in self.boundaries.rows.iter() {
            source_roots(&row.envelope, &mut roots);
            roots.extend(row.owning_service);
        }
        for row in &self.external_providers.rows {
            source_roots(&row.envelope, &mut roots);
        }
        self.static_contracts.service_roots(&mut roots);
        let roots = crate::reach::source_closure(self.typed, context, &roots)?;
        let mut bindings = Vec::with_capacity(roots.len());
        for source in roots {
            let symbol = self
                .typed
                .service_reaches
                .definition(source)
                .ok_or_else(|| unavailable(context, "service declaration is absent"))?
                .symbol;
            let declaration = self
                .typed
                .trait_definition_by_symbol(symbol)
                .filter(|declaration| declaration.is_boundary)
                .ok_or_else(|| {
                    unavailable(context, "service does not name its exact boundary trait")
                })?;
            let id = self.declared_service(context, declaration)?;
            bindings.push((source, id));
        }
        let services =
            crate::reach::ServiceMap::prepare(self.typed, context, &self.services, bindings)?;
        let contracts = self.callables.contracts.keys().collect::<Vec<_>>();
        for contract in contracts {
            let row = self.callables.contracts.get_mut(contract).ok_or_else(|| {
                unavailable(
                    context,
                    "callable contract disappeared while projecting its envelope",
                )
            })?;
            row.signature.reaches =
                crate::Ceiling::Authored(services.row(context, &row.envelope.service_reach)?);
            row.signature.invokes = crate::Ceiling::Authored(
                services
                    .envelope(context, &row.envelope)?
                    .invocations
                    .into_iter()
                    .filter_map(|entry| match entry {
                        terminal_psi::TerminalCallableInvocation::Service(service) => Some(service),
                        terminal_psi::TerminalCallableInvocation::Parameter(_) => None,
                    })
                    .collect(),
            );
        }
        let boundaries = self.boundaries.rows.keys().collect::<Vec<_>>();
        for boundary in boundaries {
            let row = self.boundaries.rows.get_mut(boundary).ok_or_else(|| {
                unavailable(
                    context,
                    "boundary disappeared while projecting its envelope",
                )
            })?;
            row.signature.reaches =
                crate::Ceiling::Authored(services.row(context, &row.envelope.service_reach)?);
            row.signature.invokes = crate::Ceiling::Authored(
                services
                    .envelope(context, &row.envelope)?
                    .invocations
                    .into_iter()
                    .filter_map(|entry| match entry {
                        terminal_psi::TerminalCallableInvocation::Service(service) => Some(service),
                        terminal_psi::TerminalCallableInvocation::Parameter(_) => None,
                    })
                    .collect(),
            );
        }
        Ok(services)
    }
}
fn source_roots(envelope: &SourceEnvelope, roots: &mut Vec<ServiceReachId>) {
    roots.extend_from_slice(&envelope.service_reach);
    roots.extend(
        envelope
            .invocations
            .iter()
            .filter_map(|target| match target {
                Invocation::Service(service) => Some(*service),
                Invocation::Parameter(_) => None,
            }),
    );
}

use semantic_vocabulary::ServiceId;
use typed_trees::trait_definition::{AppliedTraitScope, TraitApplicationArguments};

use crate::{Unserved, refuse};

impl<'t> Program<'t> {
    pub(super) fn service(
        &mut self,
        caller: MachineId,
        scope: AppliedTraitScope<'t>,
    ) -> Checked<ServiceId> {
        let unavailable =
            |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
        if let Some((service, _)) = self
            .services
            .iter()
            .find(|(_, (known, _))| known.same_application(self.typed, &scope))
        {
            return Ok(service);
        }
        if self.resolving_services.contains(&scope.declaration.symbol) {
            return Err(unavailable("cyclic applied service parent declarations"));
        }
        if scope.portable_application_identity(self.typed).is_none() {
            return Err(unavailable(
                "service has no complete exact application identity",
            ));
        }
        self.resolving_services.push(scope.declaration.symbol);
        let parents = (|| {
            scope
                .parent_scopes(self.typed)
                .map_err(unavailable)?
                .into_iter()
                .map(|parent| self.service(caller, parent))
                .collect::<Checked<Vec<_>>>()
        })();
        self.resolving_services.pop();
        let mut parents = parents?;
        parents.sort_unstable();
        parents.dedup();
        Ok(self.services.push((scope, parents)))
    }

    pub(super) fn declared_service(
        &mut self,
        caller: MachineId,
        owner: &'t typed_trees::trait_definition::TraitDefinition,
    ) -> Checked<ServiceId> {
        let scope = AppliedTraitScope::new(
            self.typed,
            owner,
            TraitApplicationArguments {
                lexical_owner: owner.symbol,
                enclosing_data_applications: &[],
                arguments: &[],
                lifetimes: &[],
            },
        )
        .ok_or_else(|| {
            refuse(
                caller,
                Construct::Machine,
                Unserved::Declaration("boundary service needs its complete applied declaration"),
            )
        })?;
        self.service(caller, scope)
    }

    /// What `owner`'s header says about service reach: its `reaches` clause
    /// as the services it names and their boundary parents, or `Inferred`
    /// when it wrote none. One reader for a checked body, a requirement and
    /// a boundary requirement.
    ///
    /// effects.md, "Declarations": "`reaches A + B` bounds service reach by
    /// the named boundary services and their boundary parents"; "Boundary
    /// traits supply identities that may be named in `reaches`"; "Ordinary
    /// traits are not service identities." The typed trees keep one authored
    /// row per written clause, with no target for `reaches none`, so a
    /// clause is told from an omission by the row and never by an empty
    /// set. A ceiling bound by an installation (`reaches <= Bound`) is an
    /// abstract row, which no slice reads yet.
    pub(super) fn reach_ceiling(
        &mut self,
        caller: MachineId,
        owner: symbols::SymbolHandle,
    ) -> Checked<crate::Ceiling> {
        let typed = self.typed;
        let unserved = |what| refuse(caller, Construct::Machine, Unserved::Machine(what));
        let Some(owner) = crate::reach::authored_ceiling_owner(typed, caller, owner)? else {
            return Ok(crate::Ceiling::Inferred);
        };
        let mut named = Vec::new();
        for row in typed.authored_service_reach_rows_for(owner) {
            if row.installation_bound {
                return Err(unserved("service reach bound by an installation"));
            }
            named.extend(row.targets.iter().map(|target| target.service));
        }
        Ok(crate::Ceiling::Authored(self.named_row(
            caller,
            named,
            crate::body::Axis::Reach,
        )?))
    }

    /// What a header says about synchronous entry: its `invokes` rows as
    /// the requirement identities they name. `bodyless` is a requirement,
    /// boundary or not.
    ///
    /// effects.md, "Direct synchronous invocation": on a checked body an
    /// omitted clause means "Infer and publish its entries" and a written
    /// one bounds them; on a bodyless requirement omission is "No
    /// synchronous entry", an empty written row. "A row names a parameter
    /// path or a requirement identity." A parameter path stands for the
    /// binding a call supplies, so charging a caller for it needs the
    /// actual substituted at each call. The complete SourceEnvelope retains
    /// that origin while this signature row records only service entries.
    pub(super) fn entry_ceiling(
        &mut self,
        caller: MachineId,
        rows: &[typed_trees::signature::AuthoredInvocation],
        parameters: &[typed_trees::signature::StateParameter],
        bodyless: bool,
    ) -> Checked<crate::Ceiling> {
        use typed_trees::signature::AuthoredInvocationTarget as Named;
        if rows.is_empty() && !bodyless {
            return Ok(crate::Ceiling::Inferred);
        }
        let mut named = Vec::with_capacity(rows.len());
        for row in rows {
            match row.target {
                Named::Service(requirement) => named.push(requirement),
                Named::Parameter { ordinal, symbol }
                    if symbol.is_valid()
                        && parameters
                            .get(ordinal as usize)
                            .is_some_and(|formal| formal.symbol == symbol) =>
                {
                    // The full envelope retains this exact formal origin;
                    // this header row is only its service projection.
                }
                Named::Parameter { .. } => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Machine("invokes has no exact formal origin"),
                    ));
                }
                Named::Unresolved => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Typing("invokes names nothing this program declares"),
                    ));
                }
            }
        }
        Ok(crate::Ceiling::Authored(self.named_row(
            caller,
            named,
            crate::body::Axis::Entries,
        )?))
    }

    /// The row a clause on `axis` writes by naming `named`: the one reading
    /// of a written ceiling. Each name must be a boundary trait, whose
    /// applied declaration is its service identity in the module's catalog.
    /// A reach row is closed under boundary parents; an entry row is not
    /// (`invokes_entry_ceiling.md`, "No parent closure").
    ///
    /// For reach a name that is not a boundary trait is a mistake of the
    /// program ("Ordinary traits are not service identities"). For entries
    /// it may be a valid requirement identity ("Route and direction decide
    /// what a row may name, not declaration kind"), which the catalog has
    /// no identity for yet, so it is not served.
    fn named_row(
        &mut self,
        caller: MachineId,
        named: Vec<symbols::SymbolHandle>,
        axis: crate::body::Axis,
    ) -> Checked<Vec<ServiceId>> {
        let typed = self.typed;
        let mut row = Vec::with_capacity(named.len());
        for service in named {
            let declaration = typed
                .trait_definition_by_symbol(service)
                .filter(|declaration| declaration.is_boundary)
                .ok_or_else(|| {
                    refuse(
                        caller,
                        Construct::Machine,
                        match axis {
                            crate::body::Axis::Reach => Unserved::Typing(
                                "reaches names something that is not a boundary trait",
                            ),
                            crate::body::Axis::Entries => Unserved::Machine(
                                "invokes naming a requirement that is not a boundary trait",
                            ),
                        },
                    )
                })?;
            row.push(self.declared_service(caller, declaration)?);
        }
        Ok(match axis {
            crate::body::Axis::Reach => self.with_parents(row),
            crate::body::Axis::Entries => {
                row.sort_unstable();
                row.dedup();
                row
            }
        })
    }

    /// `services` and every boundary parent of one, strictly increasing.
    /// effects.md: "parents(R) subset-of normalize(R)".
    fn with_parents(&self, mut services: Vec<ServiceId>) -> Vec<ServiceId> {
        let mut next = 0;
        while let Some(service) = services.get(next).copied() {
            next += 1;
            if let Some((_, parents)) = self.services.get(service) {
                for parent in parents {
                    if !services.contains(parent) {
                        services.push(*parent);
                    }
                }
            }
        }
        services.sort_unstable();
        services.dedup();
        services
    }
}
