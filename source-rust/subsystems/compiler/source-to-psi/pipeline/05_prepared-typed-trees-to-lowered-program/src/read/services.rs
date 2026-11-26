//! Keep each callable's authored permissions separate from what its body reaches.
//! Private omitted axes are inferred from actual calls, never from callers or a
//! list of eligible implementations. Published axes are checked as ceilings.

use crate::publication::reach::{SourceEnvelope, SourceInvocation as Invocation};
use crate::source_state::TypedTrees;
use crate::source_state::signature::{
    AuthoredInvocation, AuthoredInvocationTarget, StateSignature,
};
use arena::IdTable;
use semantic_vocabulary::ServiceReachId;
use semantic_vocabulary::ServiceReachRowId;
use semantic_vocabulary::{ContractId, MachineId};
use source_declarations::TerminationGuarantee;
use source_declarations::TerminationInterface;
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{TerminalCallableTermination, TerminalModule};

use super::Program;
use crate::body::{
    Body, CallTarget, Expression, ExpressionKind as E, RecordType, Signature, Statement,
    StorageShape, Target, ValueType,
};

pub(crate) fn unavailable(machine: MachineId, detail: impl Into<String>) -> Refusal {
    Refusal {
        source_span: None,
        site: terminal_check_tables::RefusalSite::Machine(machine, Construct::Machine),
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
    Ok(typed.service_reaches.canonical_id(id))
}

pub(super) fn service_closure(
    typed: &TypedTrees,
    context: MachineId,
    roots: &[ServiceReachId],
) -> Checked<Vec<ServiceReachId>> {
    crate::publication::reach::source_closure(typed, context, roots)
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
    authored: arena::RootRows<'_, AuthoredInvocation>,
    parameters: arena::RootRows<'_, crate::source_state::signature::StateParameter>,
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
                let runtime = parameters
                    .slice(0..source)
                    .iter()
                    .filter(|formal| formal.relevance == language_core::BindingRelevance::Relevant)
                    .count();
                Invocation::Parameter(u32::try_from(runtime).map_err(|_| {
                    unavailable(context, "invocation runtime parameter coordinate overflow")
                })?)
            }
            AuthoredInvocationTarget::AppliedService(id) => {
                Invocation::Service(service(typed, context, id)?)
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

    /// Keep the original bounds on the published machine. The working signature
    /// also carries inferred axes for calls whose service reach is still open.
    pub(crate) fn declared_operational_ceilings(
        &self,
        id: MachineId,
    ) -> Option<crate::body::OperationalCeilings> {
        let policy = self.declared_envelope_policies.get(id)?.as_ref()?;
        Some(crate::body::OperationalCeilings {
            suspension: policy.suspends,
            blocking: policy.blocks,
        })
    }

    pub(crate) fn finish_operational_envelopes(
        &mut self,
        bodies: &IdTable<MachineId, Body>,
        components: &crate::components::Components,
    ) -> Checked<()> {
        let declaration_policies = self.declaration_policies()?;
        let mut abstract_requirements = self.requirements.clone();
        abstract_requirements.sort_unstable();
        let mut known = IdTable::default();
        let mut policies = IdTable::default();
        let mut published = IdTable::default();
        let mut calls = IdTable::default();
        for (id, symbol) in self.symbols.iter() {
            let signature = self
                .signatures
                .get(id)
                .ok_or_else(|| unavailable(id, "callable has no retained signature"))?;
            let operational = signature.operational;
            let policy = if let Some(policy) = self
                .declared_envelope_policies
                .get(id)
                .and_then(Option::as_ref)
            {
                policy.clone()
            } else if let Some(machine) = self.typed.machine_by_symbol(*symbol) {
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
                let Some([state]) = states.as_array() else {
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
                    reach: crate::publication::reach::written_ceiling(self.typed, id, *symbol)?,
                    invokes: if !authored.is_empty() {
                        Some(invocations(self.typed, id, authored, parameters)?)
                    } else {
                        None
                    },
                    suspends: operational.suspension,
                    blocks: operational.blocking,
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
            // Completion may run again after more callees are discovered.
            // Keep the original policy so a prior inference never becomes an
            // authored ceiling on that later pass.
            while self.declared_envelope_policies.len() < id.get() as usize {
                self.declared_envelope_policies.push(None);
            }
            let original = self
                .declared_envelope_policies
                .get_mut(id)
                .ok_or_else(|| unavailable(id, "callable lost its source policy coordinate"))?;
            if original.is_none() {
                *original = Some(policy.clone());
            }
            known.push(if abstract_requirements.binary_search(&id).is_ok() {
                KnownAxes::of_policy(&policy)
            } else {
                KnownAxes::ALL
            });
            published.push(policy.initial());
            policies.push(policy);
            let body = terminal_check_tables::row(bodies, id, id, Construct::Machine)?;
            let found = CallCollector::for_body(id, signature, body, &self.records)?;
            if !components.members(id)?.is_empty()
                && found
                    .iter()
                    .any(|call| !call.static_reach_arguments.is_empty())
            {
                return Err(unavailable(
                    id,
                    "a static callback substitution on a recursive edge needs its retained call correspondence",
                ));
            }
            calls.push(found);
        }
        loop {
            let mut changed = false;
            for (id, uses) in calls.iter() {
                let mut reached = empty();
                let mut reached_known = KnownAxes::ALL;
                for invocation in uses {
                    if let Some(required) = invocation.required_contract {
                        reached_known.intersect(self.declaration_axes(
                            id,
                            required,
                            &declaration_policies,
                            &known,
                        )?);
                    } else if let CallTarget::Machine(callee) = invocation.target {
                        reached_known.intersect(*known.get(*callee).ok_or_else(|| {
                            unavailable(id, "callee lost its operational inference state")
                        })?);
                    }
                    let target_signature = self.source_call_signature(
                        id,
                        invocation.target,
                        invocation.required_contract,
                    )?;
                    let envelope = if let Some(required) = invocation.required_contract {
                        self.declaration_envelopes.get(required).ok_or_else(|| {
                            unavailable(
                                id,
                                "selected public contract has no complete envelope coordinate",
                            )
                        })?
                    } else {
                        match invocation.target {
                            CallTarget::Machine(callee) => terminal_check_tables::row(
                                &published,
                                *callee,
                                id,
                                Construct::Machine,
                            )?,
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
                                // An actual that joins several callable
                                // formals hands the callee each of them.
                                let forwarded: Vec<_> = invocation
                                    .parameter_arguments
                                    .iter()
                                    .filter(|(argument, _)| *argument == source_position)
                                    .map(|(_, parameter)| Invocation::Parameter(*parameter))
                                    .collect();
                                if !forwarded.is_empty() {
                                    union(&mut reached.invocations, &forwarded);
                                    continue;
                                }
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
                                let actual =
                                    self.callables.contracts.get(contract).ok_or_else(|| {
                                        unavailable(
                                            id,
                                            "invoked actual has no retained callable contract",
                                        )
                                    })?;
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
                    for parameter in &invocation.selected_parameters {
                        union(
                            &mut reached.invocations,
                            &[Invocation::Parameter(*parameter)],
                        );
                    }
                }
                let policy = policies
                    .get(id)
                    .ok_or_else(|| unavailable(id, "callable has no header policy"))?;
                let next_known = if abstract_requirements.binary_search(&id).is_ok() {
                    KnownAxes::of_policy(policy)
                } else {
                    reached_known.with_policy(policy)
                };
                let previous_known = known.get_mut(id).ok_or_else(|| {
                    unavailable(id, "callable lost its operational inference state")
                })?;
                if *previous_known != next_known {
                    *previous_known = next_known;
                    changed = true;
                }
                let next = policy.complete(reached);
                let previous = published
                    .get_mut(id)
                    .ok_or_else(|| unavailable(id, "callable has no envelope row"))?;
                if *previous != next {
                    *previous = next;
                    changed = true;
                }
            }
            for (contract, policy) in declaration_policies.iter() {
                let selected = self.declaration_selections.get(contract).ok_or_else(|| {
                    unavailable(
                        MachineId::new(1).expect("nonzero diagnostic"),
                        "public contract lost its selection coordinate",
                    )
                })?;
                let Some((caller, callee, ..)) = selected else {
                    continue;
                };
                let reached = published
                    .get(*callee)
                    .ok_or_else(|| {
                        unavailable(
                            *caller,
                            "selected public contract has no actual body envelope",
                        )
                    })?
                    .clone();
                let next = policy.complete(reached);
                let previous = self
                    .declaration_envelopes
                    .get_mut(contract)
                    .ok_or_else(|| {
                        unavailable(
                            *caller,
                            "selected public contract lost its complete envelope coordinate",
                        )
                    })?;
                if *previous != next {
                    *previous = next;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // Public invocation contracts must be finite at publication. An open
        // static requirement elsewhere in the body graph is independent.
        for (contract, selected) in self.declaration_selections.iter() {
            if let Some((caller, ..)) = selected
                && !self
                    .declaration_axes(*caller, contract, &declaration_policies, &known)?
                    .is_complete()
            {
                return Err(unavailable(
                    *caller,
                    "selected public contract depends on an unknown operational bound of its implementation",
                ));
            }
        }
        // The completed envelopes are carried into Terminal Psi. Its checker
        // validates acknowledgements, source positions and suspension evidence
        // on the actual emitted calls.
        // A private body's progress belongs to this checked closure. Its
        // ranked cycles still owe their comparisons; a silent exported or
        // selected requirement cannot borrow that private evidence.
        let private = self.symbols.map(|id, symbol| {
            !self.requirements.contains(&id)
                && self
                    .typed
                    .machine_by_symbol(*symbol)
                    .is_some_and(|machine| !machine.is_public && machine.body_is_present)
        });
        let mut local_termination = policies.map(|id, policy| {
            matches!(
                policy.termination,
                TerminalCallableTermination::Terminates { .. }
            ) || (private.get(id) == Some(&true)
                && !components.reaches_unranked_cycle(crate::components::index(id)))
        });
        // Callee components are settled first. A ranked cycle may assume its
        // own progress only while every member and outgoing call supports it.
        for (members, _) in components.callee_first() {
            let mut complete = true;
            for &id in members {
                let uses = calls
                    .get(id)
                    .ok_or_else(|| unavailable(id, "progress subject has no invocation row"))?;
                if local_termination.get(id) != Some(&true)
                    || !uses
                        .iter()
                        .all(|invocation| call_terminates(self, invocation, &local_termination))
                {
                    complete = false;
                    break;
                }
            }
            if !complete {
                for &id in members {
                    if private.get(id) == Some(&true)
                        && !matches!(
                            policies.get(id).map(|policy| &policy.termination),
                            Some(TerminalCallableTermination::Terminates { .. })
                        )
                    {
                        *local_termination
                            .get_mut(id)
                            .ok_or_else(|| unavailable(id, "termination subject has no row"))? =
                            false;
                    }
                }
            }
        }
        for (id, eligible) in private.iter() {
            if *eligible && local_termination.get(id) == Some(&true) {
                published
                    .get_mut(id)
                    .ok_or_else(|| unavailable(id, "private progress subject has no envelope"))?
                    .termination = TerminalCallableTermination::Terminates {
                    premises: Vec::new(),
                };
            }
        }
        let mut checking_envelopes = IdTable::default();
        for (id, envelope) in published.iter() {
            if matches!(
                envelope.termination,
                TerminalCallableTermination::Terminates { .. }
            ) && calls
                .get(id)
                .ok_or_else(|| unavailable(id, "termination subject has no invocations"))?
                .iter()
                .any(|invocation| !call_terminates(self, invocation, &local_termination))
            {
                return Err(unavailable(
                    id,
                    "termination promise invokes a callable without a termination guarantee",
                ));
            }
            let complete = *known
                .get(id)
                .ok_or_else(|| unavailable(id, "callable lost its operational inference state"))?;
            // An inferred boolean is useful even when an unrelated reach axis
            // remains open. Never turn that partial result into a full envelope.
            let signature = self
                .signatures
                .get_mut(id)
                .ok_or_else(|| unavailable(id, "callable lost its retained signature"))?;
            if complete.suspends {
                signature.operational.suspension = Some(envelope.suspends);
            }
            if complete.blocks {
                signature.operational.blocking = Some(envelope.blocks);
            }
            checking_envelopes.push(
                (!complete.is_complete()
                    && complete.invokes
                    && complete.suspends
                    && complete.blocks)
                    .then(|| crate::callables::static_contracts::StaticSourceEnvelope {
                        service_reach: if complete.reach {
                            crate::callables::static_contracts::SourceServiceCeiling::Closed(
                                envelope.service_reach.clone(),
                            )
                        } else {
                            crate::callables::static_contracts::SourceServiceCeiling::Open
                        },
                        invocations: envelope.invocations.clone(),
                        suspends: envelope.suspends,
                        blocks: envelope.blocks,
                        termination: envelope.termination.clone(),
                    }),
            );
            if !complete.is_complete() {
                continue;
            }
            self.callables
                .bind_machine_envelope(id, envelope.clone())
                .map_err(|reason| unavailable(id, reason))?;
        }
        self.checking_envelopes = checking_envelopes;
        Ok(())
    }

    fn declaration_axes(
        &self,
        caller: MachineId,
        contract: ContractId,
        policies: &IdTable<ContractId, Policy>,
        known: &IdTable<MachineId, KnownAxes>,
    ) -> Checked<KnownAxes> {
        let policy = policies.get(contract).ok_or_else(|| {
            unavailable(caller, "selected public contract lost its declared policy")
        })?;
        match self.declaration_selections.get(contract) {
            Some(Some((_, selected, ..))) => known
                .get(*selected)
                .map(|axes| axes.with_policy(policy))
                .ok_or_else(|| {
                    unavailable(
                        caller,
                        "selected public contract lost its implementation bounds",
                    )
                }),
            Some(None) => Ok(KnownAxes::of_policy(policy)),
            None => Err(unavailable(
                caller,
                "public contract lost its selection coordinate",
            )),
        }
    }

    fn declaration_policies(&self) -> Checked<IdTable<ContractId, Policy>> {
        let diagnostic = MachineId::new(1).expect("nonzero diagnostic identity");
        if self.declaration_signatures.len() != self.declaration_sources.len()
            || self.declaration_signatures.len() != self.declaration_selections.len()
            || self.declaration_signatures.len() != self.declaration_envelopes.len()
        {
            return Err(unavailable(
                diagnostic,
                "public contracts lost their exact source coordinate tables",
            ));
        }
        let mut policies = IdTable::default();
        for (contract, signature) in self.declaration_signatures.iter() {
            let envelope = self
                .declaration_envelopes
                .get(contract)
                .ok_or_else(|| unavailable(diagnostic, "public contract has no source envelope"))?;
            let policy = Policy {
                reach: matches!(signature.reaches, crate::Ceiling::Authored(_))
                    .then(|| envelope.service_reach.clone()),
                invokes: matches!(signature.invokes, crate::Ceiling::Authored(_))
                    .then(|| envelope.invocations.clone()),
                suspends: signature.operational.suspension,
                blocks: signature.operational.blocking,
                termination: envelope.termination.clone(),
            };
            if matches!(self.declaration_selections.get(contract), Some(None))
                && (policy.reach.is_none()
                    || policy.invokes.is_none()
                    || policy.suspends.is_none()
                    || policy.blocks.is_none())
            {
                return Err(unavailable(
                    diagnostic,
                    "unselected public contract retains an open operational axis",
                ));
            }
            if policies.push(policy) != contract {
                return Err(unavailable(
                    diagnostic,
                    "public contract policy lost its exact coordinate",
                ));
            }
        }
        Ok(policies)
    }

    pub(super) fn retain_requirement_policy(
        &mut self,
        id: MachineId,
        signature: &StateSignature,
        source_owner: symbols::SymbolHandle,
    ) -> Checked<()> {
        let parameters = self
            .typed
            .state_parameters
            .span(signature.parameters)
            .ok_or_else(|| unavailable(id, "requirement policy has a stale parameter span"))?;
        let authored = self
            .typed
            .signature_invokes
            .span(signature.invokes)
            .ok_or_else(|| unavailable(id, "requirement policy has a stale invocation span"))?;
        // A concrete member determines open and installation-bound rows. A
        // fixed ceiling on the requirement remains its own applied contract.
        let reach =
            if signature.service_reach_is_installation_bound && source_owner != signature.symbol {
                crate::publication::reach::written_ceiling(self.typed, id, source_owner)?
            } else {
                match crate::publication::reach::authored_source_ceiling(
                    self.typed,
                    id,
                    signature.symbol,
                    signature.service_reach_row,
                )? {
                    None if source_owner != signature.symbol => {
                        crate::publication::reach::written_ceiling(self.typed, id, source_owner)?
                    }
                    declared => declared,
                }
            };
        let policy = Policy {
            reach,
            invokes: Some(invocations(self.typed, id, authored, parameters)?),
            suspends: Some(signature.suspends),
            blocks: Some(signature.blocks),
            termination: termination(id, &signature.termination_guarantee)?,
        };
        while self.declared_envelope_policies.len() < id.get() as usize {
            self.declared_envelope_policies.push(None);
        }
        let saved = self
            .declared_envelope_policies
            .get_mut(id)
            .ok_or_else(|| unavailable(id, "requirement policy has no exact callable row"))?;
        if saved.is_some() {
            return Err(unavailable(id, "requirement policy was already retained"));
        }
        *saved = Some(policy);
        Ok(())
    }

    pub(super) fn retain_pure_requirement_policy(&mut self, id: MachineId) -> Checked<()> {
        let policy = Policy {
            reach: Some(Vec::new()),
            invokes: Some(Vec::new()),
            suspends: Some(false),
            blocks: Some(false),
            termination: TerminalCallableTermination::Terminates {
                premises: Vec::new(),
            },
        };
        while self.declared_envelope_policies.len() < id.get() as usize {
            self.declared_envelope_policies.push(None);
        }
        let saved = self
            .declared_envelope_policies
            .get_mut(id)
            .ok_or_else(|| unavailable(id, "requirement policy has no exact callable row"))?;
        if saved.is_some() {
            return Err(unavailable(id, "requirement policy was already retained"));
        }
        *saved = Some(policy);
        Ok(())
    }

    pub(crate) fn retain_requirement_schema(&mut self, id: MachineId) -> Checked<()> {
        use crate::callables::static_contracts::{
            SourceSchema, SourceSchemaRequirement, SourceServiceCeiling, StaticSourceEnvelope,
        };
        if !self.requirements.contains(&id) {
            return Err(unavailable(
                id,
                "an abstract schema names no actual requirement",
            ));
        }
        let policy = self
            .declared_envelope_policies
            .get(id)
            .and_then(Option::as_ref)
            .ok_or_else(|| unavailable(id, "abstract requirement lost its source policy"))?;
        let signature = self
            .signatures
            .get(id)
            .ok_or_else(|| unavailable(id, "abstract requirement lost its source header"))?;
        let declaration = *self
            .symbols
            .get(id)
            .ok_or_else(|| unavailable(id, "abstract requirement lost its source declaration"))?;
        let abstract_types = signature
            .parameters
            .iter()
            .copied()
            .chain(signature.result)
            .filter_map(|value| match value {
                crate::body::ValueType::Rigid(rigid) => Some(rigid.0),
                _ => None,
            })
            .max()
            .map_or(Some(0), |last| last.checked_add(1))
            .ok_or_else(|| unavailable(id, "abstract requirement type namespace overflow"))?;
        let source = SourceSchemaRequirement {
            instance: Some(id),
            declaration,
            abstract_types,
            signature: Some(signature.clone()),
            envelope: StaticSourceEnvelope {
                service_reach: match &policy.reach {
                    Some(row) => SourceServiceCeiling::Closed(row.clone()),
                    None => SourceServiceCeiling::Open,
                },
                invocations: policy.invokes.clone().ok_or_else(|| {
                    unavailable(id, "abstract requirement lost its invocation bound")
                })?,
                suspends: policy.suspends.ok_or_else(|| {
                    unavailable(id, "abstract requirement lost its suspension bound")
                })?,
                blocks: policy.blocks.ok_or_else(|| {
                    unavailable(id, "abstract requirement lost its blocking bound")
                })?,
                termination: policy.termination.clone(),
            },
        };
        let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        match self
            .static_contracts
            .schemas
            .binary_search_by_key(&key(declaration), |schema| key(schema.declaration))
        {
            Ok(position) => {
                let schema = &mut self.static_contracts.schemas[position];
                if schema
                    .requirements
                    .last()
                    .is_some_and(|row| row.instance.is_some_and(|earlier| earlier >= id))
                {
                    return Err(unavailable(
                        id,
                        "abstract requirement schema was already retained",
                    ));
                }
                schema.requirements.push(source);
            }
            Err(position) => self.static_contracts.schemas.insert(
                position,
                SourceSchema {
                    declaration,
                    requirements: vec![source],
                    bindings: Vec::new(),
                },
            ),
        }
        Ok(())
    }
}

fn call_terminates(
    program: &Program<'_>,
    invocation: &Use<'_>,
    named: &IdTable<MachineId, bool>,
) -> bool {
    if let Some(required) = invocation.required_contract {
        return program
            .declaration_envelopes
            .get(required)
            .is_some_and(|envelope| {
                matches!(
                    envelope.termination,
                    TerminalCallableTermination::Terminates { .. }
                )
            });
    }
    match invocation.target {
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

/// A false bit means the corresponding inferred value is still unknown.
/// Body cycles start known and lose knowledge only through an open dependency;
/// the numeric/set fixed point remains separate from this completeness check.
#[derive(Clone, Copy, PartialEq, Eq)]
struct KnownAxes {
    reach: bool,
    invokes: bool,
    suspends: bool,
    blocks: bool,
}

impl KnownAxes {
    const ALL: Self = Self {
        reach: true,
        invokes: true,
        suspends: true,
        blocks: true,
    };

    fn of_policy(policy: &Policy) -> Self {
        Self {
            reach: policy.reach.is_some(),
            invokes: policy.invokes.is_some(),
            suspends: policy.suspends.is_some(),
            blocks: policy.blocks.is_some(),
        }
    }

    fn intersect(&mut self, dependency: Self) {
        self.reach &= dependency.reach;
        self.invokes &= dependency.invokes;
        self.suspends &= dependency.suspends;
        self.blocks &= dependency.blocks;
    }

    fn with_policy(self, policy: &Policy) -> Self {
        let authored = Self::of_policy(policy);
        Self {
            reach: self.reach || authored.reach,
            invokes: self.invokes || authored.invokes,
            suspends: self.suspends || authored.suspends,
            blocks: self.blocks || authored.blocks,
        }
    }

    fn is_complete(self) -> bool {
        self == Self::ALL
    }
}

#[derive(Clone)]
pub(super) struct Policy {
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
    fn complete(&self, reached: SourceEnvelope) -> SourceEnvelope {
        // Keep authored bounds even when a call exceeds them. The lowered-form
        // checker rejects that actual call at its source position on every axis.
        SourceEnvelope {
            service_reach: self.reach.clone().unwrap_or(reached.service_reach),
            invocations: self.invokes.clone().unwrap_or(reached.invocations),
            suspends: self.suspends.unwrap_or(reached.suspends),
            blocks: self.blocks.unwrap_or(reached.blocks),
            termination: self.termination.clone(),
        }
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
    /// The caller's callable formals the invoked value may be.
    selected_parameters: Vec<u32>,
    parameter_arguments: Vec<(usize, u32)>,
    target: &'a CallTarget,
    required_contract: Option<ContractId>,
    arguments: &'a [Expression],
    static_reach_arguments: &'a [terminal_psi::static_reach::StaticReachArgument],
}

struct CallCollector<'a> {
    sites: Vec<Use<'a>>,
    // Which caller parameters supplied the value now in each slot: one, or
    // each formal a `match` joined there. Empty when another value may be
    // there.
    parameters: Vec<Vec<u32>>,
}

impl<'a> CallCollector<'a> {
    fn for_body(
        id: MachineId,
        signature: &Signature,
        body: &'a Body,
        records: &IdTable<semantic_vocabulary::StructuralTypeId, RecordType>,
    ) -> Checked<Vec<Use<'a>>> {
        let mut collector = Self {
            sites: Vec::new(),
            parameters: vec![Vec::new(); body.slots.len()],
        };
        for (slot, ty) in signature.parameters.iter().enumerate() {
            let scalar = match ty {
                ValueType::Scalar(scalar, _) => Some(scalar),
                ValueType::Record(record, _, _) => match records
                    .get(*record)
                    .map(|record| &record.shape)
                {
                    Some(StorageShape::PrimitiveScalar { scalar_type, .. }) => Some(scalar_type),
                    _ => None,
                },
                ValueType::Rigid(_) => None,
            };
            if matches!(scalar, Some(semantic_vocabulary::ScalarType::Callable(_)))
                && let Some(position) = signature.runtime_parameter_position(slot)
                && let Some(binding) = collector.parameters.get_mut(slot)
            {
                *binding =
                    vec![u32::try_from(position).map_err(|_| {
                        unavailable(id, "invocation parameter coordinate overflow")
                    })?];
            }
        }
        for statement in &body.statements {
            collect_statement(statement, &mut collector);
            // Evaluate the right-hand side against the old binding before replacing it.
            match statement {
                Statement::Observe { slot, value }
                | Statement::Bind { slot, value }
                | Statement::Store {
                    destination:
                        Expression {
                            kind: E::Slot(slot),
                            ..
                        },
                    value,
                    ..
                } => {
                    let parameter = collector.parameters(value);
                    *collector
                        .parameters
                        .get_mut(*slot)
                        .ok_or_else(|| unavailable(id, "binding names no body slot"))? = parameter;
                }
                _ => {}
            }
        }
        Ok(collector.sites)
    }

    /// The caller's formals `expression` may be, or none when some value it
    /// may be came from elsewhere. A value `match` may be any of its arms,
    /// and a named machine is no formal: invoking it enters only what its
    /// callable type permits.
    fn parameters(&self, expression: &Expression) -> Vec<u32> {
        let mut found = Vec::new();
        if self.traced(expression, &mut found) {
            found.sort_unstable();
            found.dedup();
            found
        } else {
            Vec::new()
        }
    }

    fn traced(&self, expression: &Expression, found: &mut Vec<u32>) -> bool {
        match &expression.kind {
            E::Match { arms, default, .. } => {
                arms.iter().all(|(_, arm)| self.traced(arm, found))
                    && default
                        .as_deref()
                        .is_none_or(|default| self.traced(default, found))
            }
            E::Block {
                statements,
                value: Some(value),
            } if statements.is_empty() => self.traced(value, found),
            E::MachineValue { .. } => true,
            E::Requalify(value) => self.traced(value, found),
            _ => match Self::slot(expression).and_then(|slot| self.parameters.get(slot)) {
                Some(parameters) if !parameters.is_empty() => {
                    found.extend_from_slice(parameters);
                    true
                }
                _ => false,
            },
        }
    }

    fn slot(expression: &Expression) -> Option<usize> {
        match &expression.kind {
            E::Slot(slot) => Some(*slot),
            E::Requalify(value) => Self::slot(value),
            _ => None,
        }
    }
}

fn collect_statement<'a>(statement: &'a Statement, calls: &mut CallCollector<'a>) {
    match statement {
        Statement::Observe { value, .. }
        | Statement::Bind { value, .. }
        | Statement::Evaluate {
            expression: value, ..
        }
        | Statement::Decompose { subject: value } => collect_expression(value, calls),
        Statement::Store {
            destination, value, ..
        } => {
            collect_expression(destination, calls);
            collect_expression(value, calls);
        }
        // Capturing an existing reference aliases its already evaluated
        // binding; its initializer's calls were charged at that earlier Bind.
        Statement::CaptureReference { .. } => {}
        Statement::Witness { .. } | Statement::Declare { .. } => {}
        Statement::Dispatch {
            subject,
            arms,
            default,
            ..
        } => {
            collect_expression(subject, calls);
            for (selector, target, _) in arms {
                if let Some(condition) = selector.condition.as_deref() {
                    collect_expression(condition, calls);
                }
                collect_target(target, calls);
            }
            if let Some((target, _)) = default {
                collect_target(target, calls);
            }
        }
        Statement::Arm { guard, target, .. } => {
            if let Some(guard) = guard.condition() {
                collect_expression(guard, calls);
            }
            collect_target(target, calls);
        }
    }
}
fn collect_target<'a>(target: &'a Target, calls: &mut CallCollector<'a>) {
    if let Target::Value(value) = target {
        collect_expression(value, calls);
    }
}
fn collect_expression<'a>(expression: &'a Expression, calls: &mut CallCollector<'a>) {
    match &expression.kind {
        E::TheoremApplication { .. } => {}
        E::Block {
            statements, value, ..
        } => {
            for statement in statements {
                collect_statement(statement, calls);
            }
            if let Some(value) = value {
                collect_expression(value, calls);
            }
        }
        E::Call {
            callee,
            required_contract,
            arguments,
            static_reach_arguments,
            ..
        } => {
            let selected_parameters = if let CallTarget::Selected { callable, .. } = callee {
                let parameters = calls.parameters(callable);
                collect_expression(callable, calls);
                parameters
            } else {
                Vec::new()
            };
            let mut parameter_arguments = Vec::new();
            for (position, argument) in arguments.iter().enumerate() {
                for parameter in calls.parameters(argument) {
                    parameter_arguments.push((position, parameter));
                }
                collect_expression(argument, calls);
            }
            calls.sites.push(Use {
                selected_parameters,
                parameter_arguments,
                target: callee,
                required_contract: *required_contract,
                arguments,
                static_reach_arguments,
            });
        }
        E::OwnedTemporary { value: base, .. }
        | E::Field { base, .. }
        | E::CollectionView { base, .. }
        | E::StorageLocation { place: base, .. }
        | E::CollectionMeasure { base, .. }
        | E::ByteSequencePredicate { base, .. }
        | E::Establish { operand: base, .. }
        | E::Payload { base, .. }
        | E::IsCase { subject: base, .. }
        | E::Not(base)
        | E::BitwiseNot(base)
        | E::Cast(base)
        | E::Requalify(base)
        | E::Referent(base) => collect_expression(base, calls),
        E::Borrow { place, access, .. } => {
            collect_expression(place, calls);
            // An exclusive borrower can replace the callback before its next read.
            if access.is_exclusive()
                && let Some(slot) = CallCollector::slot(place)
                && let Some(binding) = calls.parameters.get_mut(slot)
            {
                binding.clear();
            }
        }
        E::Record { fields, .. } | E::Case { fields, .. } => {
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
            for (selector, arm) in arms {
                if let Some(pattern) = selector.test() {
                    collect_expression(pattern, calls);
                }
                collect_expression(arm, calls);
            }
            if let Some(default) = default {
                collect_expression(default, calls);
            }
        }
        E::TestMembership { subject: left, predicate: right, .. }
        | E::Strict { left, right, .. } | E::ShortCircuit { left, right, .. } => {
            collect_expression(left, calls);
            collect_expression(right, calls);
        }
        E::PackedInteger { operands: elements, .. } | E::Array { elements } => {
            for element in elements {
                collect_expression(element, calls);
            }
        }
        E::CollectionSubslice { base, start, end } => {
            collect_expression(base, calls);
            if let Some(start) = start {
                collect_expression(start, calls);
            }
            if let Some(end) = end {
                collect_expression(end, calls);
            }
        }
        E::Index { base, index } | E::ByteElement { base, index, .. } => {
            collect_expression(base, calls);
            collect_expression(index, calls);
        }
        E::Atomic(operation) => {
            for operand in operation.inputs() {
                collect_expression(operand, calls);
            }
        }
        E::SavedSubject(_)
        | E::Zero
        | E::Float(_)
        | E::ErasedApplication(_)
        | E::ScalarApplication { .. }
        | E::ByteLiteral(_)
        | E::Integer(_)
        | E::Boolean(_)
        | E::Slot(_)
        | E::MachineValue { .. } => {}
    }
}

impl Program<'_> {
    pub(crate) fn prepare_service_bindings(
        &mut self,
        context: MachineId,
    ) -> Checked<Vec<(ServiceReachId, semantic_vocabulary::ServiceId)>> {
        let mut roots = Vec::new();
        for (_, row) in self.callables.contracts.iter() {
            source_roots(&row.envelope, &mut roots);
        }
        for (_, row) in self.callables.envelopes.iter() {
            if let Some(row) = row {
                source_roots(row, &mut roots);
            }
        }
        for (_, row) in self.declaration_envelopes.iter() {
            source_roots(row, &mut roots);
        }
        for (_, row) in self.boundaries.rows.iter() {
            source_roots(&row.envelope, &mut roots);
        }
        for row in &self.external_providers.rows {
            source_roots(&row.envelope, &mut roots);
        }
        self.static_contracts.service_roots(&mut roots);
        let roots = crate::publication::reach::source_closure(self.typed, context, &roots)?;
        let mut bindings = Vec::with_capacity(roots.len());
        for source in roots {
            let id = self.applied_source_service(context, source)?;
            bindings.push((source, id));
        }
        Ok(bindings)
    }

    pub(crate) fn service_map(
        &mut self,
        context: MachineId,
    ) -> Checked<crate::publication::reach::ServiceMap> {
        let bindings = self.prepare_service_bindings(context)?;
        let services = if self.services.is_empty() {
            if !bindings.is_empty() {
                return Err(unavailable(
                    context,
                    "service binding has no namespace declaration",
                ));
            }
            crate::publication::reach::ServiceMap::default()
        } else {
            let catalog = self
                .shared
                .calling_catalog(self.typed, self.purpose)
                .as_ref()
                .map(std::sync::Arc::clone)
                .map_err(|reason| unavailable(context, reason.clone()))?;
            let catalog = catalog
                .bind(
                    crate::source_state::provider_declarations::ProviderPlanningContext::new(
                        self.typed,
                        self.purpose,
                    ),
                )
                .map_err(|error| unavailable(context, error.message))?;
            crate::publication::reach::ServiceMap::prepare(
                self.typed,
                context,
                &self.services,
                bindings,
                catalog,
            )?
        };
        let contracts = self.callables.contracts.keys().collect::<Vec<_>>();
        for contract in contracts {
            let row = self.callables.contracts.get_mut(contract).ok_or_else(|| {
                unavailable(
                    context,
                    "callable contract disappeared while projecting its envelope",
                )
            })?;
            row.signature.reaches = crate::Ceiling::Authored(
                services.row(context, &row.envelope.service_reach)?,
            );
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
            row.signature.reaches = crate::Ceiling::Authored(
                services.row(context, &row.envelope.service_reach)?,
            );
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

use crate::source_state::trait_definition::{AppliedTraitScope, TraitApplicationArguments};
use semantic_vocabulary::ServiceId;

use crate::{Unserved, refuse};

impl<'t> Program<'t> {
    pub(super) fn service(
        &mut self,
        caller: MachineId,
        scope: AppliedTraitScope<'t>,
    ) -> Checked<ServiceId> {
        let unavailable =
            |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
        if !scope.declaration.is_boundary {
            return Err(unavailable(
                "ordinary trait cannot declare a service identity",
            ));
        }
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
        let parents: Checked<Vec<ServiceId>> = (|| {
            let declared = scope.parent_scopes(self.typed).map_err(unavailable)?;
            let mut services = Vec::new();
            let mut has_ordinary_parent = false;
            for parent in declared {
                if parent.declaration.is_boundary {
                    services.push(self.service(caller, parent)?);
                } else {
                    has_ordinary_parent = true;
                }
            }
            if has_ordinary_parent {
                self.require_boundary_parent_contract(caller, &scope)?;
            }
            Ok(services)
        })();
        self.resolving_services.pop();
        let mut parents = parents?;
        parents.sort_unstable();
        parents.dedup();
        // Service lineage retains the exact application as metadata. Contract
        // readers separately admit the source headers they actually check.
        Ok(self.services.push((scope, parents)))
    }

    fn applied_source_service(
        &mut self,
        caller: MachineId,
        source: ServiceReachId,
    ) -> Checked<ServiceId> {
        let typed = self.typed;
        let scope = typed.service_reach_scope(source).ok_or_else(|| {
            let name = typed
                .service_reaches
                .definition(source)
                .map_or("<absent>", |row| row.name.as_str());
            unavailable(
                caller,
                format!("boundary service `{name}` requires its complete declared application"),
            )
        })?;
        self.service(caller, scope)
    }

    pub(super) fn declared_service(
        &mut self,
        caller: MachineId,
        owner: &'t crate::source_state::trait_definition::TraitDefinition,
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

    pub(super) fn reach_ceiling(
        &mut self,
        caller: MachineId,
        owner: symbols::SymbolHandle,
        row: ServiceReachRowId,
    ) -> Checked<crate::Ceiling> {
        let Some(sources) =
            crate::publication::reach::authored_source_ceiling(self.typed, caller, owner, row)?
        else {
            return Ok(crate::Ceiling::Inferred);
        };
        let mut services = sources
            .into_iter()
            .map(|source| self.applied_source_service(caller, source))
            .collect::<Checked<Vec<_>>>()?;
        services.sort_unstable();
        services.dedup();
        Ok(crate::Ceiling::Authored(services))
    }

    /// Prepare each selected callable and its binder ceiling in the same
    /// catalog as the executable body. An unused binding needs its own
    /// checked declaration but contributes no call or service reach.
    pub(crate) fn application_binders(
        &mut self,
        owner: MachineId,
    ) -> Checked<crate::declare::BinderReach> {
        use crate::source_state::static_applications::StaticMachineBindingValue;

        let typed = self.typed;
        let unserved = |what| refuse(owner, Construct::Machine, Unserved::Declaration(what));
        let symbol = *self
            .symbols
            .get(owner)
            .ok_or_else(|| unserved("a callback application lost its owner"))?;
        let Some(instantiation) = typed.machine_instantiation(symbol).map_err(unserved)? else {
            return Ok(Vec::new());
        };
        let bindings = typed
            .expression_table
            .static_machine_application_bindings(instantiation.binding_application());
        let mut bounds = Vec::new();
        for binding in bindings {
            let StaticMachineBindingValue::Machine(argument) = &binding.value else {
                continue;
            };
            let contract = typed
                .retained_static_machine_contract(binding.parameter)
                .ok_or_else(|| unserved("a static callback binding has no source requirement"))?;
            let signature = contract.signature();
            let bound =
                match self.reach_ceiling(owner, signature.symbol, signature.service_reach_row)? {
                    crate::Ceiling::Inferred => None,
                    crate::Ceiling::Authored(bound) => Some(bound),
                };
            let selected = typed
                .machine_by_symbol(argument.symbol)
                .or_else(|| {
                    typed
                        .unique_machine_by_entry_state_symbol(argument.symbol)
                        .map(|(machine, _)| machine)
                })
                .ok_or_else(|| {
                    unserved("a static callback binding has no exact selected machine")
                })?;
            if typed.machine_callable_parameters(selected).is_empty() {
                self.machine(selected.symbol)?;
            }
            bounds.push((binding.parameter, bound));
        }
        Ok(bounds)
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
        rows: arena::RootRows<'_, crate::source_state::signature::AuthoredInvocation>,
        parameters: arena::RootRows<'_, crate::source_state::signature::StateParameter>,
        bodyless: bool,
    ) -> Checked<crate::Ceiling> {
        use crate::source_state::signature::AuthoredInvocationTarget as Named;
        if rows.is_empty() && !bodyless {
            return Ok(crate::Ceiling::Inferred);
        }
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            match row.target {
                Named::AppliedService(service) => {
                    entries.push(self.applied_source_service(caller, service)?);
                }
                Named::Service(symbol) => {
                    let source = self
                        .typed
                        .service_reaches
                        .id_for_symbol(symbol)
                        .ok_or_else(|| {
                            unavailable(
                                caller,
                                "the named effect does not select a boundary service",
                            )
                        })?;
                    entries.push(self.applied_source_service(caller, source)?);
                }
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
                // "A row names a parameter path or a requirement identity",
                // and typing found that nothing the row's source can see
                // has its name, so it names neither in every program. A
                // row whose name something visible has may name one where
                // typing does not follow it yet.
                Named::Undeclared => {
                    return Err(crate::reject(
                        caller,
                        crate::SourceOrigin::of(row.source_span),
                        format!(
                            "`invokes {}` names no parameter and no requirement: a name no declaration visible \
                             here has is an error wherever it appears",
                            row.name.as_str()
                        ),
                    ));
                }
                Named::Unresolved => {
                    return Err(refuse(
                        caller,
                        Construct::Machine,
                        Unserved::Typing(
                            "invokes naming a declaration typing does not select as a requirement",
                        ),
                    ));
                }
            }
        }
        entries.sort_unstable();
        entries.dedup();
        Ok(crate::Ceiling::Authored(entries))
    }

    /// Notes that a Build selection awaits a provider for `service`.
    pub(super) fn awaits(&mut self, service: ServiceId) {
        if !self.readiness.contains(&service) {
            self.readiness.push(service);
        }
    }

    /// The trait that declares each awaited requirement, by its exact
    /// owner's identity, interned in `module`. The provider the Build host
    /// later installs is matched on this identity and on the application
    /// identity of the service row, so both are the ones a provider's own
    /// module states for the same declarations.
    pub(crate) fn publish_readiness(
        &self,
        caller: MachineId,
        module: &mut TerminalModule,
    ) -> Checked<Vec<(ServiceId, terminal_psi::NominalDeclarationIdentity)>> {
        if self.readiness.is_empty() {
            return Ok(Vec::new());
        }
        let unavailable =
            |reason| refuse(caller, Construct::Machine, Unserved::Declaration(reason));
        let sources = source_admission::source_identity::toolchain_source_identities(
            self.typed.symbols.source_files(),
        )
        .map_err(|_| {
            unavailable("an awaited requirement has a toolchain source with no exact identity")
        })?;
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let rows = self
            .readiness
            .iter()
            .map(|service| {
                let (scope, _) = self
                    .services
                    .get(*service)
                    .ok_or("an awaited requirement lost its service row")?;
                crate::declare::nominal(
                    self.typed,
                    &sources,
                    scope.declaration.symbol,
                    &mut identities,
                )
                .map(|identity| {
                    (
                        *service,
                        terminal_psi::NominalDeclarationIdentity::new(identity),
                    )
                })
            })
            .collect::<Result<Vec<_>, &'static str>>();
        module.identities = identities.build();
        rows.map_err(unavailable)
    }
}
