//! A completed call changes the actual storage selected at its entry. Its body
//! can corroborate a caller's existing promises. A public normal contract may
//! conserve a fact on its exact captured original subject. Neither route treats
//! entry annotations or a replacement pointer as proof about an older pointee.

mod completed_outputs;
mod primitive_writes;
mod provider_observations;
mod reference_forwarding;
pub use provider_observations::require_boundary_lender_preservation;
mod write_conservation;

use super::*;
use terminal_psi::find_structural_type;

#[derive(Clone)]
struct ParameterWrite {
    parameter: usize,
    steps: Vec<DescriptorContentsStep>,
    performed: Result<write_conservation::PerformedWrite, &'static str>,
}

pub(super) struct NormalReturnProof {
    location: DescriptorLocation,
    contracts: Vec<PayloadMembershipContract>,
}

impl NormalReturnProof {
    pub(super) fn restore(
        self,
        types: &[StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        before: &State,
        after: &mut State,
    ) {
        // A whole current payload can discharge its complete original roster.
        // A proof for one selected child cannot certify the other children or
        // an ancestor duty after the root's history became unavailable.
        if !self.location.path.is_empty()
            || before
                .unavailable_preservation
                .contains(&self.location.root)
        {
            return;
        }
        let Some(required) = before.values.get(&self.location.root) else {
            return;
        };
        if self.contracts.is_empty()
            || !self.contracts.into_iter().all(|contract| {
                contract
                    .presented(types, domains, required.carrier)
                    .is_some_and(|mut proven| {
                        proven.qualifications.sort();
                        proven.qualifications.dedup();
                        proven.projected_qualifications.sort();
                        proven.projected_qualifications.dedup();
                        let mut whole = required.qualifications.clone();
                        whole.sort();
                        whole.dedup();
                        let mut projected = required.projected_qualifications.clone();
                        projected.sort();
                        projected.dedup();
                        whole == proven.qualifications
                            && projected == proven.projected_qualifications
                    })
            })
        {
            return;
        }
        let Some(current) = after.values.get_mut(&self.location.root) else {
            return;
        };
        if current.carrier != required.carrier {
            return;
        }
        // Preserve nominal promises only. The pre-call scalar observations and
        // equalities describe the old contents and stay retired.
        current.qualifications = required.qualifications.clone();
        current.projected_qualifications = required.projected_qualifications.clone();
        after
            .unavailable_preservation
            .retain(|root| *root != self.location.root);
    }
}

pub(super) struct NormalPayloadContext<'module> {
    module: &'module TerminalModule,
    graph: &'module [DescriptorMachine],
    writes: BTreeMap<MachineId, Result<Vec<ParameterWrite>, &'static str>>,
    writing: BTreeSet<MachineId>,
    posts: BTreeMap<(MachineId, PlaceId), Result<Vec<PayloadMembershipContract>, &'static str>>,
    posting: BTreeSet<MachineId>,
    completing: BTreeSet<MachineId>,
    complete_entries: BTreeSet<MachineId>,
}

impl<'module> NormalPayloadContext<'module> {
    pub(super) fn new(
        module: &'module TerminalModule,
        graph: &'module [DescriptorMachine],
    ) -> Self {
        Self {
            module,
            graph,
            writes: BTreeMap::new(),
            writing: BTreeSet::new(),
            posts: BTreeMap::new(),
            posting: BTreeSet::new(),
            completing: BTreeSet::new(),
            complete_entries: BTreeSet::new(),
        }
    }

    fn executable_body(
        &self,
        identity: MachineId,
    ) -> Result<&'module TerminalMachine, &'static str> {
        let mut bodies = self
            .module
            .machines
            .iter()
            .filter(|body| body.id == identity);
        let body = bodies.next().ok_or("the normal payload body is absent")?;
        if bodies.next().is_some() {
            return Err("the normal payload body is ambiguous");
        }
        if body
            .blocks
            .iter()
            .flat_map(|block| block.terminator.cleanup_rosters())
            .flat_map(|(_, actions)| actions)
            .any(|action| action.invoked_machine().is_some())
        {
            return Err(
                "the normal payload body has an ordered nominal cleanup effect without a replay association",
            );
        }
        Ok(body)
    }

    pub(super) fn call_writes(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
    ) -> Result<Vec<DescriptorContentsQuery>, &'static str> {
        let mut queries = Vec::new();
        for effect in self.call_write_effects(analysis, caller, position)? {
            if !queries.contains(&effect.query) {
                queries.push(effect.query);
            }
        }
        Ok(queries)
    }

    fn call_write_effects(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
    ) -> Result<Vec<write_conservation::CallWriteEffect>, &'static str> {
        let graph = self.graph;
        let machine =
            descriptor_machine(graph, caller).ok_or("the call write has no caller graph")?;
        let call = machine
            .calls
            .get(&position)
            .ok_or("the call write has no performed invocation")?;
        let bodies = match &call.target {
            DescriptorInvocationTarget::Machine(body) => vec![*body],
            DescriptorInvocationTarget::Public {
                effects_machine: Some(body),
                ..
            } => vec![*body],
            DescriptorInvocationTarget::Public { realizations, .. } if !realizations.is_empty() => {
                realizations.clone()
            }
            _ => return Err("the call has no complete executable write body"),
        };
        // Private entry assumptions are usable only across an actual direct
        // call. An effects body chosen for a public boundary is not its public
        // nominal contract.
        let scalar_bindings = self
            .direct_call_bindings(caller, position)
            .filter(|bindings| call.callee() == Some(bindings.callee.id));
        let mut result = Vec::new();
        for body in bodies {
            for effect in self.body_writes(graph, analysis, body)? {
                let mut actual = call
                    .argument_contents
                    .get(effect.parameter)
                    .and_then(Option::as_ref)
                    .ok_or("the call write has an unresolved actual contents binding")?
                    .clone();
                actual.steps.extend(effect.steps);
                actual
                    .validate(
                        &self.module.structural_types,
                        *machine
                            .place_types
                            .get(&actual.root)
                            .ok_or("the call write actual has no carrier")?,
                    )
                    .ok_or("the call write actual projection has no complete typed path")?;
                let performed = if let Some(bindings) = &scalar_bindings {
                    effect
                        .performed
                        .and_then(|performed| performed.bound(bindings.scalar_substitutions()))
                } else {
                    Err("public effect selection does not authenticate private writer evidence")
                };
                result.push(write_conservation::CallWriteEffect {
                    query: actual,
                    performed,
                });
            }
        }
        Ok(result)
    }

    pub(super) fn call_posts(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        caller: MachineId,
        position: DescriptorPosition,
    ) -> Result<Vec<NormalReturnProof>, &'static str> {
        let graph = self.graph;
        let mut callers = self.module.machines.iter().filter(|body| body.id == caller);
        let machine = callers.next().ok_or("the normal call has no caller body")?;
        if callers.next().is_some() {
            return Err("the normal call has ambiguous caller bodies");
        }
        let block = machine
            .blocks
            .iter()
            .find(|block| block.id == position.0)
            .ok_or("the normal call has no owning block")?;
        let operation = block
            .operations
            .get(position.1 as usize)
            .ok_or("the normal call operation is absent")?;
        let expressions = super::super::payload_predicates::TerminalScalarExpressions::new(machine);
        let bindings = super::super::scalar_contracts::terminal_call_bindings(
            self.module,
            machine,
            operation,
            &expressions,
        )?;
        let call = descriptor_machine(graph, caller)
            .and_then(|body| body.calls.get(&position))
            .ok_or("the normal call has no invocation graph")?;
        if call.callee() != Some(bindings.callee.id) {
            return Err("the normal call body and invocation disagree");
        }
        let mut proofs = Vec::new();
        for parameter in bindings.parameters {
            if parameter.access == StructuralAccess::Owned {
                continue;
            }
            let query = parameter
                .contents
                .as_ref()
                .ok_or("the normal call actual has unresolved contents")?;
            if call
                .argument_contents
                .get(parameter.position)
                .and_then(Option::as_ref)
                != Some(query)
            {
                return Err("the normal call actual contents disagree with its invocation");
            }
            let targets = analysis
                .payload_targets(caller, position, query)
                .ok_or("the normal call actual has unresolved backing alternatives")?;
            if targets.is_empty() {
                return Err("the normal call actual has no backing alternative");
            }
            let contracts = match self.body_posts(
                graph,
                analysis,
                bindings.callee.id,
                parameter.formal,
            ) {
                Ok(contracts) => contracts,
                Err(reason) => {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        eprintln!(
                            "CHECKSITE normal-parameter-post-unavailable caller={caller:?} position={position:?} callee={:?} formal={:?} reason={reason}",
                            bindings.callee.id, parameter.formal
                        );
                    }
                    continue;
                }
            };
            for target in targets {
                let DescriptorPayloadTarget::Storage(location) = target else {
                    return Err("the normal call actual is still symbolic at this caller");
                };
                proofs.push(NormalReturnProof {
                    location,
                    contracts: contracts.clone(),
                });
            }
        }
        Ok(proofs)
    }

    fn body_writes(
        &mut self,
        graph: &[DescriptorMachine],
        analysis: &mut DescriptorAnalysis<'_>,
        identity: MachineId,
    ) -> Result<Vec<ParameterWrite>, &'static str> {
        if let Some(result) = self.writes.get(&identity) {
            return result.clone();
        }
        if !self.writing.insert(identity) {
            return Err("the normal write summary has a recursive dependency");
        }
        let result = self.collect_body_writes(graph, analysis, identity);
        self.writing.remove(&identity);
        self.writes.insert(identity, result.clone());
        result
    }

    fn collect_body_writes(
        &mut self,
        graph: &[DescriptorMachine],
        analysis: &mut DescriptorAnalysis<'_>,
        identity: MachineId,
    ) -> Result<Vec<ParameterWrite>, &'static str> {
        let body = self.executable_body(identity)?;
        let mut history = PayloadMembershipHistory::new(
            &self.module.structural_types,
            &self.module.structural_domains,
            self.graph,
            body,
        );
        let mut preservation = PayloadMembershipHistory::for_preservation(
            &self.module.structural_types,
            &self.module.structural_domains,
            self.graph,
            body,
        );
        let machine =
            descriptor_machine(graph, identity).ok_or("the normal write body graph is absent")?;
        let feasible = analysis.flow_feasibility(identity);
        let mut effects = Vec::new();
        for (position, step) in &machine.flow {
            match feasible.get(position) {
                Some(FlowFeasibility::ProvedImpossible) => continue,
                Some(FlowFeasibility::Possible) => {}
                _ => return Err("a normal body write has unknown reachability"),
            }
            let mut targets = Vec::new();
            let performed = if machine.primitive_payload_writes.contains(position) {
                self.performed_primitive_write(analysis, identity, *position)
                    .map(write_conservation::PerformedWrite::Primitive)
            } else {
                self.performed_store_before(
                    analysis,
                    &mut history,
                    &mut preservation,
                    identity,
                    *position,
                ).map(write_conservation::PerformedWrite::Payload)
            }
                .inspect_err(|reason| {
                    if !step.derivations.is_empty()
                        && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
                    {
                        eprintln!("CHECKSITE normal-performed-write-unavailable machine={identity:?} position={position:?} reason={reason}");
                    }
            });
            if machine.calls.contains_key(position) {
                for effect in self.call_write_effects(analysis, identity, *position)? {
                    targets.extend(
                        analysis
                            .payload_targets(identity, *position, &effect.query)
                            .ok_or("a nested normal call write has unresolved targets")?
                            .into_iter()
                            .map(|target| (target, effect.performed.clone())),
                    );
                }
            } else {
                if step.may_escape || machine.unresolved_contents_writes.contains(position) {
                    return Err("the normal body has an unresolved contents write");
                }
                for query in machine
                    .payload_invalidations
                    .get(position)
                    .into_iter()
                    .flatten()
                {
                    targets.extend(
                        query
                            .as_ref()
                            .and_then(|query| analysis.payload_targets(identity, *position, query))
                            .ok_or("a normal body payload write has unresolved targets")?
                            .into_iter()
                            .map(|target| {
                                let proof = performed.as_ref().map_err(|reason| *reason)
                                    .and_then(|proof| {
                                        (query.as_ref() == Some(&DescriptorContentsQuery::from_location(proof.destination())))
                                            .then(|| proof.clone())
                                            .ok_or("the performed writer belongs to another invalidation footprint")
                                    });
                                (target, proof)
                            }),
                    );
                }
            }
            for (actual, destination) in &step.derivations {
                if machine.payload_producers.get(position) == Some(&destination.root)
                    || machine.storage_derivations.contains(&(
                        *position,
                        actual.clone(),
                        destination.clone(),
                    ))
                {
                    continue;
                }
                targets.extend(
                    analysis
                        .payload_targets(
                            identity,
                            *position,
                            &DescriptorContentsQuery::from_location(destination),
                        )
                        .ok_or("a normal body store has unresolved selected payload storage")?
                        .into_iter()
                        .map(|target| {
                            (
                                target,
                                performed.as_ref().map_err(|reason| *reason).and_then(|proof| {
                                    proof.payload().is_some_and(|payload| payload.source == *actual && payload.destination == *destination)
                                        .then(|| proof.clone()).ok_or("the performed payload belongs to another structural derivation")
                                }),
                            )
                        }),
                );
            }
            for (target, performed) in targets {
                let query = match target {
                    DescriptorPayloadTarget::Storage(location) => {
                        let mut rows = machine
                            .places
                            .iter()
                            .filter(|row| row.place == location.root);
                        let row = rows
                            .next()
                            .ok_or("a normal body write has no storage owner")?;
                        if rows.next().is_some() {
                            return Err("a normal body write has ambiguous storage owners");
                        }
                        if matches!(
                            row.definition,
                            DescriptorDefinition::Local
                                | DescriptorDefinition::Arrival {
                                    access: StructuralAccess::Owned
                                }
                                | DescriptorDefinition::Parameter {
                                    access: StructuralAccess::Owned,
                                    ..
                                }
                        ) {
                            let carrier = *machine
                                .place_types
                                .get(&location.root)
                                .ok_or("the local write has no carrier")?;
                            for end in 0..=location.path.len() {
                                let tip = crate::descriptors::descriptor_lifetimes::descriptor_location_type(
                                    &self.module.structural_types, carrier, &location.path[..end],
                                );
                                let Some(tip) = tip else {
                                    if end < location.path.len() {
                                        return Err(
                                            "the local write has an unavailable storage prefix",
                                        );
                                    }
                                    continue;
                                };
                                if matches!(
                                    find_structural_type(&self.module.structural_types, tip)
                                        .ok_or("the local write carrier is unavailable")?
                                        .shape,
                                    StructuralTypeShape::Record {
                                        storage: terminal_psi::StructuralRecordStorage::AccessView(
                                            _
                                        )
                                            | terminal_psi::StructuralRecordStorage::Accessor(_),
                                        ..
                                    }
                                ) {
                                    return Err(
                                        "the normal body write uses an external access interpretation",
                                    );
                                }
                            }
                            continue;
                        }
                        DescriptorContentsQuery::from_location(&location)
                    }
                    DescriptorPayloadTarget::Incoming(query) => query,
                };
                let mut rows = machine.places.iter().filter(|row| row.place == query.root);
                let row = rows
                    .next()
                    .ok_or("the normal body write has no formal owner")?;
                let DescriptorDefinition::Parameter { position, .. } = row.definition else {
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        let owner = match row.definition {
                            DescriptorDefinition::Arrival { access } => match access {
                                StructuralAccess::Owned => "owned-arrival",
                                _ => "borrowed-arrival",
                            },
                            DescriptorDefinition::Local => "local",
                            DescriptorDefinition::Literal => "literal",
                            DescriptorDefinition::Parameter { .. } => unreachable!(),
                        };
                        eprintln!(
                            "CHECKSITE normal-write-source-unavailable machine={identity:?} position={position:?} target={query:?} owner={owner}"
                        );
                    }
                    return Err("the normal body write has no parameter-relative source");
                };
                if rows.next().is_some() {
                    return Err("the normal body write has an ambiguous formal owner");
                }
                // Distinct performed writes must survive even if they select
                // the same parameter. One good replacement cannot hide another
                // writer whose payload is unavailable.
                effects.push(ParameterWrite {
                    parameter: position,
                    steps: query.steps,
                    performed,
                });
            }
        }
        Ok(effects)
    }

    fn body_posts(
        &mut self,
        graph: &[DescriptorMachine],
        analysis: &mut DescriptorAnalysis<'_>,
        identity: MachineId,
        root: PlaceId,
    ) -> Result<Vec<PayloadMembershipContract>, &'static str> {
        if let Some(result) = self.posts.get(&(identity, root)) {
            return result.clone();
        }
        if !self.posting.insert(identity) {
            return Err("the normal payload summary has a recursive dependency");
        }
        let result = (|| {
            let body = self.executable_body(identity)?;
            self.body_writes(graph, analysis, identity)?;
            let mut history = PayloadMembershipHistory::for_preservation(
                &self.module.structural_types,
                &self.module.structural_domains,
                graph,
                body,
            );
            history
                .preserved_normal_exit_contracts_in(analysis, root, Some(self))
                .map(|exits| exits.into_iter().map(|(_, contract)| contract).collect())
        })();
        self.posting.remove(&identity);
        self.posts.insert((identity, root), result.clone());
        result
    }
}

#[cfg(test)]
mod tests;
