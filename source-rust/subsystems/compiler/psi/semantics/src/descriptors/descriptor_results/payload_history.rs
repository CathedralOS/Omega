pub(crate) mod entry_snapshots;
pub(crate) mod premise_table;

use premise_table::{PremiseTable, SharedPremise};

use super::PayloadScalarObservation;
use super::contents::select_payload_memberships;
use super::payload_predicates;
use super::payload_predicates::observation_type;
use super::payload_reachability;
use super::payload_reachability::incoming_flow;
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_path;
use crate::qualification_path_carrier;
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Default, PartialEq, Eq)]
struct PayloadContracts(BTreeMap<PlaceId, Arc<PayloadMembershipContract>>);

impl PayloadContracts {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn get(&self, root: &PlaceId) -> Option<&PayloadMembershipContract> {
        self.0.get(root).map(Arc::as_ref)
    }
    fn get_mut(&mut self, root: &PlaceId) -> Option<&mut PayloadMembershipContract> {
        self.0.get_mut(root).map(Arc::make_mut)
    }
    fn insert(&mut self, root: PlaceId, contract: PayloadMembershipContract) {
        self.0.insert(root, Arc::new(contract));
    }
    fn remove(&mut self, root: &PlaceId) -> Option<Arc<PayloadMembershipContract>> {
        self.0.remove(root)
    }
    fn values_mut(&mut self) -> impl Iterator<Item = &mut Arc<PayloadMembershipContract>> {
        self.0.values_mut()
    }
}

#[cfg(test)]
impl<const N: usize> From<[(PlaceId, PayloadMembershipContract); N]> for PayloadContracts {
    fn from(rows: [(PlaceId, PayloadMembershipContract); N]) -> Self {
        Self(
            rows.into_iter()
                .map(|(root, contract)| (root, Arc::new(contract)))
                .collect(),
        )
    }
}

#[cfg(test)]
impl std::ops::Index<&PlaceId> for PayloadContracts {
    type Output = PayloadMembershipContract;
    fn index(&self, root: &PlaceId) -> &Self::Output {
        self.get(root).unwrap()
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    values: PayloadContracts,
    scalar_requirements: Arc<Vec<SharedPremise>>,
    shared_requirements: Arc<Vec<SharedPremise>>,
}

impl State {
    fn publish_requirements(&mut self) {
        for contract in self.values.values_mut() {
            if !contract.scalar_requirements.iter().any(|premise| {
                self.scalar_requirements
                    .binary_search_by(|candidate| candidate.as_ref().cmp(premise))
                    .is_ok()
            }) {
                continue;
            }
            let contract = Arc::make_mut(contract);
            let mut common = self
                .scalar_requirements
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            contract.scalar_requirements.retain(|premise| {
                while common.peek().is_some_and(|candidate| *candidate < premise) {
                    common.next();
                }
                !common.peek().is_some_and(|candidate| *candidate == premise)
            });
        }
        self.shared_requirements = self.scalar_requirements.clone();
    }

    fn contract(
        &self,
        root: PlaceId,
        predicate: Option<&semantic_vocabulary::Proposition>,
    ) -> Option<PayloadMembershipContract> {
        let mut contract = self.values.get(&root)?.clone();
        extend_canonical_premises(
            &mut contract.scalar_requirements,
            &self.shared_requirements,
        );
        if let Some(predicate) = predicate
            && !contract.scalar_requirements.contains(predicate)
        {
            contract.scalar_requirements.push(predicate.clone());
        }
        Some(contract)
    }

    fn meet(&mut self, mut arrival: State) -> bool {
        let previous_shared = std::mem::take(&mut self.shared_requirements);
        let mut shared = previous_shared.clone();
        retain_common_snapshot(&mut shared, &arrival.shared_requirements);
        let mut changed = retain_common_snapshot(
            &mut self.scalar_requirements,
            &arrival.scalar_requirements,
        );
        let previous_values = self.values.len();
        self.values.0.retain(|place, contract| {
            let Some(incoming) = arrival.values.remove(place) else {
                return false;
            };
            if contract.carrier != incoming.carrier {
                return false;
            }
            let contract = Arc::make_mut(contract);
            let previous_premises =
                contract.scalar_requirements.len() + previous_shared.len();
            retain_captured_premises(
                &mut contract.scalar_requirements,
                &incoming.scalar_requirements,
                &arrival.shared_requirements,
            );
            let mut incoming_premises = incoming.scalar_requirements.clone();
            retain_common_premises(&mut incoming_premises, &previous_shared);
            extend_canonical_premises(
                &mut contract.scalar_requirements,
                &incoming_premises,
            );
            changed |=
                contract.scalar_requirements.len() + shared.len() != previous_premises;
            let previous_counts = (
                contract.qualifications.len(),
                contract.projected_qualifications.len(),
            );
            changed |= PayloadScalarObservation::intersect(
                &mut contract.scalar_observations,
                &incoming.scalar_observations,
            );
            retain_common_memberships(
                &mut contract.qualifications,
                &incoming.qualifications,
            );
            retain_common_memberships(
                &mut contract.projected_qualifications,
                &incoming.projected_qualifications,
            );
            changed |= previous_counts
                != (
                    contract.qualifications.len(),
                    contract.projected_qualifications.len(),
                );
            true
        });
        self.shared_requirements = shared;
        changed || self.values.len() != previous_values
    }
}

struct CachedHistory {
    relevant: payload_reachability::Reachability,
    snapshots: BTreeMap<DescriptorPosition, State>,
}

#[derive(Default)]
struct HistoryReplay {
    // Immutable declaration classification shared by this history's cuts.
    content_independent: Option<Vec<semantic_vocabulary::StructuralDomainId>>,
    premises: PremiseTable,
    cached: Option<CachedHistory>,
    inventory: Option<DescriptorSlotKind>,
}

enum PayloadSources<'input> {
    Terminal(&'input TerminalMachine),
    Current {
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
    },
}
impl PayloadSources<'_> {
    fn identity(&self) -> MachineId {
        match self {
            Self::Terminal(machine) => machine.id,
            Self::Current { identity, .. } => *identity,
        }
    }
    fn parameters(&self) -> &[terminal_psi::StructuralParameterDeclaration] {
        match self {
            Self::Terminal(machine) => &machine.structural_parameters,
            Self::Current { parameters, .. } => parameters,
        }
    }
    fn contract(&self, root: PlaceId) -> Option<PayloadMembershipContract> {
        match self {
            Self::Terminal(machine) => super::terminal_payload_contract(machine, root),
            Self::Current { contracts, .. } => {
                let mut rows = contracts.iter().filter(|(place, _)| *place == root);
                let (_, contract) = rows.next()?;
                rows.next().is_none().then(|| contract.clone())
            }
        }
    }
}

pub struct PayloadMembershipHistory<'input> {
    types: &'input [StructuralTypeDeclaration],
    domains: &'input [terminal_psi::StructuralDomainDeclaration],
    graph: crate::module_analyses::DescriptorRowView<'input>,
    descriptor: Option<&'input DescriptorMachine>,
    sources: PayloadSources<'input>,
    incoming: Option<Option<payload_reachability::IncomingFlow>>,
    analysis: Option<DescriptorAnalysis<'input>>,
    replay: HistoryReplay,
    entry_snapshots: Option<Vec<(PlaceId, Vec<PayloadScalarObservation>)>>,
}
impl<'input> PayloadMembershipHistory<'input> {
    fn require_analysis_context(
        &mut self,
        analysis: &DescriptorAnalysis<'_>,
    ) -> Result<(), &'static str> {
        if !analysis.uses_source_graph(self.types, self.graph) {
            return Err(
                "payload history analysis belongs to another immutable source graph",
            );
        }
        match self.replay.inventory {
            Some(inventory) if inventory != analysis.slot_kind() => {
                Err("payload history analysis changed its descriptor inventory")
            }
            Some(_) => Ok(()),
            None => {
                self.replay.inventory = Some(analysis.slot_kind());
                Ok(())
            }
        }
    }

    pub fn new(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: impl Into<crate::module_analyses::DescriptorRowView<'input>>,
        machine: &'input TerminalMachine,
    ) -> Self {
        let graph = graph.into();
        Self {
            types,
            domains,
            graph,
            descriptor: graph.find(machine.id),
            sources: PayloadSources::Terminal(machine),
            incoming: None,
            analysis: None,
            replay: HistoryReplay::default(),
            entry_snapshots: None,
        }
    }
    pub(crate) fn for_current_contracts(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: impl Into<crate::module_analyses::DescriptorRowView<'input>>,
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
    ) -> Result<Self, &'static str> {
        let graph = graph.into();
        let current = graph
            .find(identity)
            .ok_or("current payload machine is absent")?;
        for (index, (root, contract)) in contracts.iter().enumerate() {
            if current.place_types.get(root) != Some(&contract.carrier)
                || contracts[..index]
                    .iter()
                    .any(|(previous, _)| previous == root)
            {
                return Err(
                    "current payload declaration disagrees with its exact graph carrier",
                );
            }
        }
        Ok(Self {
            types,
            domains,
            graph,
            descriptor: Some(current),
            incoming: None,
            analysis: None,
            sources: PayloadSources::Current {
                identity,
                parameters,
                contracts,
            },
            replay: HistoryReplay::default(),
            entry_snapshots: None,
        })
    }

    /// Read memberships proved at the observation. In particular, a
    /// write can invalidate the bytes required by a declared default
    /// while an exclusive loan still owes its restoration.
    pub(crate) fn current_selected_payload_facts_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        self.selected_payload_at(
            analysis,
            position,
            root,
            path,
            PayloadMembershipCut::BeforeOperation,
        )
    }

    fn selected_payload_at(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
        cut: PayloadMembershipCut,
    ) -> Result<PayloadMembershipContract, &'static str> {
        self.require_analysis_context(analysis)?;
        let identity = self.sources.identity();
        let sources = &self.sources;
        let source = |place| sources.contract(place);
        let root_contract = source(root).ok_or("selected payload root is absent")?;
        let query =
            descriptor_contents_query(self.types, root, root_contract.carrier, path, None)
                .ok_or("selected payload path has no typed coordinate")?;
        let targets = analysis
            .payload_targets(identity, position, &query)
            .ok_or("selected payload backing is unresolved")?;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| {
                descriptor.and_then(|machine| incoming_flow(&machine.flow))
            })
            .as_ref();
        let mut failure = None;
        let entry_snapshots = self.entry_snapshots.as_deref().unwrap_or(&[]);
        let replay = &mut self.replay;
        let contract = payload_memberships(self.types, self.domains, targets, |place| {
            payload_contract_at_cut(
                self.types,
                self.domains,
                descriptor,
                incoming,
                analysis,
                position,
                place,
                &source,
                entry_snapshots,
                replay,
                cut,
            )
            .map_err(|reason| failure = Some(reason))
            .ok()
        });
        contract.ok_or(failure.unwrap_or("selected payload memberships are unresolved"))
    }

    pub(crate) fn current_payload_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
    ) -> Result<PayloadMembershipContract, &'static str> {
        self.require_analysis_context(analysis)?;
        let sources = &self.sources;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| {
                descriptor.and_then(|machine| incoming_flow(&machine.flow))
            })
            .as_ref();
        current_payload_contract_reusing(
            self.types,
            self.domains,
            descriptor,
            incoming,
            analysis,
            position,
            root,
            &|place| sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.replay,
        )
    }
    pub(crate) fn scalar_snapshot(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    ) -> Result<PayloadScalarObservation, &'static str> {
        let mut analysis = self
            .analysis
            .take()
            .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
        let result = self.scalar_snapshot_in(&mut analysis, position, root, path);
        self.analysis = Some(analysis);
        result
    }

    pub fn scalar_snapshot_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    ) -> Result<PayloadScalarObservation, &'static str> {
        self.require_analysis_context(analysis)?;
        if self.entry_snapshots.is_none() {
            self.entry_snapshots = Some(entry_snapshots::collect(
                self.types,
                self.sources.parameters(),
            ));
            self.replay.cached = None;
        }
        let contract = self.current_payload_contract_in(analysis, position, root)?;
        let expected = observation_type(self.types, contract.carrier, path)
            .ok_or("snapshot subject is not an exact primitive value")?;
        let mut observations = contract
            .scalar_observations
            .iter()
            .filter(|row| row.path == path);
        let observed = observations
            .next()
            .ok_or("snapshot subject has no retained value identity")?;
        if observations.next().is_some()
            || observed
                .value
                .as_ref()
                .is_some_and(|value| value.scalar_type() != expected)
            || observed
                .entry
                .as_ref()
                .is_some_and(|entry| entry.scalar_type != expected)
            || (observed.value.is_none() && observed.entry.is_none())
        {
            return Err("snapshot subject has ambiguous or mismatched scalar identity");
        }
        Ok(observed.clone())
    }
}

pub(crate) fn current_selected_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    path: &[StructuralPathSegment],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    selected_payload_contract_in(
        types,
        domains,
        &mut DescriptorAnalysis::new(types, graph),
        descriptor_machine(graph, identity),
        identity,
        position,
        root,
        path,
        source,
    )
}

/// The memberships the payload at `root.path` holds just before `position`,
/// replaying the calls and stores of `identity` that precede it. `analysis`
/// reads the descriptor rows that contain `descriptor`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn selected_payload_contract_in(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    analysis: &mut DescriptorAnalysis<'_>,
    descriptor: Option<&DescriptorMachine>,
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    path: &[StructuralPathSegment],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let root_contract = source(root).ok_or("selected payload root is absent")?;
    let query = descriptor_contents_query(types, root, root_contract.carrier, path, None)
        .ok_or("selected payload path has no typed coordinate")?;
    let targets = analysis
        .payload_targets(identity, position, &query)
        .ok_or("selected payload backing is unresolved")?;
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    let mut replay = HistoryReplay::default();
    let mut history_failure = None;
    let contract = payload_memberships(types, domains, targets, |place| {
        current_payload_contract_reusing(
            types,
            domains,
            descriptor,
            incoming.as_ref(),
            analysis,
            position,
            place,
            source,
            &[],
            &mut replay,
        )
        .map_err(|reason| history_failure = Some(reason))
        .ok()
    });
    contract.ok_or(history_failure.unwrap_or("selected payload memberships are unresolved"))
}

#[cfg(test)]
pub(crate) fn current_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut analysis = DescriptorAnalysis::new(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    current_payload_contract_reusing(
        types,
        domains,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        source,
        &[],
        &mut HistoryReplay::default(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PayloadMembershipCut {
    BeforeOperation,
    BeforeOperationWithDeclaredBytes,
    NormalCompletion,
}

pub(crate) fn completed_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    payload_contract_at_cut(
        types,
        domains,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        source,
        &[],
        &mut HistoryReplay::default(),
        PayloadMembershipCut::NormalCompletion,
    )
}

fn current_payload_contract_reusing(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
) -> Result<PayloadMembershipContract, &'static str> {
    payload_contract_at_cut(
        types,
        domains,
        descriptor,
        incoming,
        analysis,
        position,
        root,
        source,
        entry_snapshots,
        replay,
        PayloadMembershipCut::BeforeOperationWithDeclaredBytes,
    )
}

#[allow(clippy::too_many_arguments)]
fn payload_contract_at_cut(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
    cut: PayloadMembershipCut,
) -> Result<PayloadMembershipContract, &'static str> {
    let content_independent = replay.content_independent.get_or_insert_with(|| {
        terminal_psi::StructuralDomainDeclaration::content_independent_memberships(domains)
    });
    let mut failure = "descriptor machine is absent";
    let result = (|| {
        let machine = descriptor?;
        let identity = machine.id;
        failure = "payload query reachability is invalid or exceeds its finite budget";
        let mut relevant = match incoming {
            Some(incoming) => payload_reachability::predecessors_with_incoming(
                machine, incoming, position,
            )?,
            None => payload_reachability::predecessors(machine, position)?,
        };
        let premises = &mut replay.premises;
        let previous = replay.cached.take();
        let frontier = previous.as_ref().and_then(|previous| {
            relevant.compatible_frontier(&previous.relevant, &machine.flow)
        });
        let (mut snapshots, mut pending, previous_relevant) = if let Some(frontier) =
            frontier
        {
            let previous = previous.expect("frontier owns the preceding immutable history");
            (previous.snapshots, frontier, Some(previous.relevant))
        } else {
            drop(previous);
            let mut initial = State::default();
            if let Some((_, requirements)) = &machine.payload_entry_predicates {
                initial.scalar_requirements = Arc::new(
                    super::scalar_contracts::scalar_conjuncts(requirements)
                        .into_iter()
                        .map(|premise| premises.intern(premise))
                        .collect(),
                );
            }
            canonicalize_snapshot(&mut initial.scalar_requirements);
            for place in &machine.places {
                if matches!(place.definition, DescriptorDefinition::Parameter { .. }) {
                    failure = "entry payload contract is absent";
                    let mut contract = source(place.place)?;
                    if let Some((_, snapshots)) = entry_snapshots
                        .iter()
                        .find(|(root, _)| *root == place.place)
                    {
                        contract
                            .scalar_observations
                            .extend(snapshots.iter().cloned());
                    }
                    super::payload_entry::close(
                        types,
                        domains,
                        machine,
                        place.place,
                        &mut contract,
                    );
                    contract.scalar_requirements.sort();
                    contract.scalar_requirements.dedup();
                    initial.values.insert(place.place, contract);
                }
            }
            (
                BTreeMap::from([(machine.entry, initial)]),
                VecDeque::from([machine.entry]),
                None,
            )
        };
        let mut completed = None;
        let mut stores = Vec::new();
        let mut headers = Vec::new();
        let mut bindings = Vec::new();
        let mut remaining = 65_536usize;
        while let Some(current) = pending.pop_front() {
            failure = "payload history worklist exceeded its finite budget";
            remaining = remaining.checked_sub(1)?;
            if !relevant.contains(current) {
                continue;
            }
            failure = "payload history flow position is absent";
            let mut before = snapshots.get(&current)?.clone();
            if let Some(definitions) = machine.payload_scalar_definitions.get(&current) {
                retire_scalar_instances(&mut before, definitions);
            }
            let step = machine.flow.get(&current)?;
            let mut source_values = BTreeMap::new();
            for (actual, destination) in &step.derivations {
                if machine.storage_derivations.contains(&(
                    current,
                    actual.clone(),
                    destination.clone(),
                )) {
                    continue;
                }
                if let Some(contract) = before.values.get(&actual.root) {
                    source_values.entry(actual.root).or_insert_with(|| {
                        let mut contract = contract.clone();
                        extend_canonical_premises(
                            &mut contract.scalar_requirements,
                            &before.shared_requirements,
                        );
                        contract
                    });
                }
            }
            let mut after = before;
            if machine.known_invocation(&current).is_some() {
                let writes = match analysis.storage_writes(identity, current) {
                    Ok(writes) => writes,
                    Err(reason) => {
                        failure = reason;
                        return None;
                    }
                };
                match writes {
                    Some(writes) => {
                        for location in writes {
                            if let Some(contract) = after.values.get_mut(&location.root) {
                                failure = "call payload write has no typed coordinate";
                                let path = descriptor_path(
                                    types,
                                    contract.carrier,
                                    &location.path,
                                )?;
                                retire(
                                    content_independent,
                                    contract,
                                    &path,
                                    &location.path,
                                );
                            }
                        }
                    }
                    None => {
                        let private = machine.private_observation_inputs.at(current);
                        clear_except(&mut after.values, |place| private.binary_search(place).is_ok());
                    }

                }
            } else if let Some(writes) = machine.payload_invalidations.get(&current) {
                for query in writes {
                    let targets = query.as_ref().and_then(|query| {
                        analysis.payload_targets(identity, current, query)
                    });
                    match targets {
                        Some(targets) => {
                            for target in targets {
                                match target {
                                    DescriptorPayloadTarget::Storage {
                                        location,
                                        completed: None,
                                    } => {
                                        if let Some(contract) =
                                            after.values.get_mut(&location.root)
                                        {
                                            failure = "payload write path has no typed coordinate";
                                            let path = descriptor_path(
                                                types,
                                                contract.carrier,
                                                &location.path,
                                            )?;
                                            retire(
                                                content_independent,
                                                contract,
                                                &path,
                                                &location.path,
                                            );
                                        }
                                    }
                                    DescriptorPayloadTarget::Storage {
                                        completed: Some(_),
                                        ..
                                    }
                                    | DescriptorPayloadTarget::Incoming(_) => {
                                        clear(&mut after.values)
                                    }
                                }
                            }
                        }
                        None => clear(&mut after.values),
                    }
                }
            }
            stores.clear();
            for (actual, destination) in &step.derivations {
                if machine.payload_producers.get(&current) == Some(&destination.root)
                    || machine.storage_derivations.contains(&(
                        current,
                        actual.clone(),
                        destination.clone(),
                    ))
                {
                    continue;
                }
                let mut incoming = source_values.get(&actual.root).and_then(|contract| {
                    select_payload_memberships(types, domains, contract, &actual.path)
                });
                failure = "structural store backing is unresolved";
                let destinations =
                    analysis.storage_origins(identity, current, destination)?;
                // A borrowed header such as one slice element has its own
                // contract beside the storage it reaches. The store replaces
                // the value behind the header whichever member it selected.
                if destinations
                    .iter()
                    .all(|origin| origin.root != destination.root)
                {
                    headers.push((destination.clone(), incoming.clone()));
                }
                let strong = destinations.len() == 1;
                let mut destinations = destinations.into_iter().peekable();
                while let Some(destination) = destinations.next() {
                    let selected = if destinations.peek().is_some() {
                        incoming.clone()
                    } else {
                        incoming.take()
                    };
                    stores.push((destination, selected, strong));
                }
            }
            for index in 0..stores.len() {
                let destination = &stores[index].0;
                if stores.iter().enumerate().any(|(other, (candidate, _, _))| {
                    index != other
                        && candidate.root == destination.root
                        && (candidate.path.starts_with(&destination.path)
                            || destination.path.starts_with(&candidate.path))
                }) {
                    stores[index].2 = false;
                }
            }
            for (destination, incoming, strong) in stores.drain(..) {
                let Some(contract) = after.values.get_mut(&destination.root) else {
                    continue;
                };
                failure = "structural store path has no typed coordinate";
                let path = descriptor_path(types, contract.carrier, &destination.path)?;
                retire(content_independent, contract, &path, &destination.path);
                if !strong {
                    continue;
                }
                let Some(incoming) = incoming else {
                    continue;
                };
                failure = "structural store carrier does not match its actual source";
                let destination_carrier =
                    qualification_path_carrier(types.iter(), contract.carrier, &path)?;
                let incoming = incoming.presented(types, domains, destination_carrier)?;
                replace_payload(contract, incoming, &path, &destination.path);
            }
            for (header, incoming) in headers.drain(..) {
                let Some(contract) = after.values.get_mut(&header.root) else {
                    continue;
                };
                let carrier = contract.carrier;
                // Facts about the replaced value must not outlive it. A
                // header whose path cannot be typed keeps no facts at all.
                let Some(path) = descriptor_path(types, carrier, &header.path) else {
                    after.values.remove(&header.root);
                    continue;
                };
                retire(content_independent, contract, &path, &header.path);
                if let Some(incoming) = incoming.and_then(|incoming| {
                    incoming.presented(
                        types,
                        domains,
                        qualification_path_carrier(types.iter(), carrier, &path)?,
                    )
                }) {
                    replace_payload(contract, incoming, &path, &header.path);
                }
            }
            if let Some(producer) = machine.payload_producers.get(&current) {
                failure = "established payload contract is absent";
                let mut produced = source(*producer)?;
                failure = "established payload contract has a different carrier";
                if machine.place_types.get(producer) != Some(&produced.carrier) {
                    return None;
                }
                produced.scalar_requirements.sort();
                produced.scalar_requirements.dedup();
                let completion = (identity, (current.0, current.1.checked_add(1)?));
                for (actual, destination) in &step.derivations {
                    if destination.root != *producer
                        || machine.storage_derivations.contains(&(
                            current,
                            actual.clone(),
                            destination.clone(),
                        ))
                    {
                        continue;
                    }
                    if let Some(incoming) =
                        source_values.get(&actual.root).and_then(|contract| {
                            select_payload_memberships(
                                types,
                                domains,
                                contract,
                                &actual.path,
                            )
                        })
                    {
                        // A sequence constructor authenticates the exact ordinal of
                        // each operand. Plain type paths lack that storage context.
                        failure = "payload producer destination has no typed coordinate";
                        let (path, destination_carrier) = machine.resolve_location(
                            types,
                            destination,
                            Some(completion),
                        )?;
                        let Some(incoming) =
                            incoming.presented(types, domains, destination_carrier?)
                        else {
                            continue;
                        };
                        append_scalar_observations(
                            &mut produced,
                            &incoming,
                            &destination.path,
                        );
                        if path.is_empty() {
                            produced.qualifications.extend(&incoming.qualifications);
                            produced
                                .projected_qualifications
                                .extend(incoming.projected_qualifications.clone());
                        } else {
                            terminal_psi::StructuralPathQualification::append_under(
                                &incoming.qualifications,
                                &incoming.projected_qualifications,
                                &path,
                                &mut produced.projected_qualifications,
                            );
                        }
                        produced.qualifications.sort();
                        produced.qualifications.dedup();
                        produced.projected_qualifications.sort();
                        produced.projected_qualifications.dedup();
                    }
                }
                after.values.insert(*producer, produced);
            }
            drop(source_values);
            if let Some(writes) = machine.scalar_payload_writes.get(&current) {
                for (destination, value) in writes {
                    let resolved;
                    let destination = if machine.payload_producers.get(&current)
                        == Some(&destination.root)
                    {
                        destination
                    } else {
                        let Some(destinations) =
                            analysis.storage_origins(identity, current, destination)
                        else {
                            continue;
                        };
                        if destinations.len() != 1 {
                            continue;
                        }
                        resolved = destinations.into_iter().next()?;
                        &resolved
                    };
                    let Some(contract) = after.values.get_mut(&destination.root) else {
                        continue;
                    };
                    if observation_type(types, contract.carrier, &destination.path)
                        != Some(value.scalar_type())
                    {
                        continue;
                    }
                    contract.scalar_observations.retain(|observation| {
                        !observation.path.starts_with(&destination.path)
                            && !destination.path.starts_with(&observation.path)
                    });
                    contract.scalar_observations.push(PayloadScalarObservation {
                        path: destination.path.clone(),
                        value: Some(value.clone()),
                        entry: None,
                    });
                }
            }
            if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                for (location, value) in reads {
                    if let Some(contract) = after.values.get_mut(&location.root)
                        && let Some(equality) =
                            observe_scalar(contract, &location.path, value)
                    {
                        Arc::make_mut(&mut after.scalar_requirements)
                            .push(premises.intern(equality));
                    }
                }
            }
            if let Some(requirements) = machine.payload_scalar_postconditions.get(&current)
                && !requirements.is_empty()
            {
                Arc::make_mut(&mut after.scalar_requirements).extend(
                    requirements
                        .iter()
                        .map(|premise| premises.intern_ref(premise)),
                );
            }
            canonicalize_snapshot(&mut after.scalar_requirements);
            after.publish_requirements();
            for contract in after.values.values_mut() {
                if !payload_predicates::closure_may_change(types, domains, contract) {
                    continue;
                }
                payload_predicates::close_with_shared_requirements(
                    types,
                    domains,
                    Arc::make_mut(contract),
                    &after.shared_requirements,
                    None,
                );
            }
            if cut == PayloadMembershipCut::NormalCompletion && current == position {
                completed = Some(after.clone());
            }
            let mut successors = step
                .successors
                .iter()
                .enumerate()
                .filter(|(ordinal, edge)| {
                    relevant.includes_edge(current, *ordinal)
                        && !previous_relevant
                            .as_ref()
                            .is_some_and(|previous| previous.contains(edge.target))
                })
                .peekable();
            while let Some((ordinal, edge)) = successors.next() {
                let mut arrival = if successors.peek().is_some() {
                    after.clone()
                } else {
                    std::mem::take(&mut after)
                };
                let edge_predicate = machine
                    .payload_edge_predicates
                    .get(&current)
                    .and_then(|rows| rows.get(ordinal))
                    .and_then(Option::as_ref);
                if let Some(predicate) = edge_predicate {
                    Arc::make_mut(&mut arrival.scalar_requirements).extend(
                        super::scalar_contracts::scalar_conjuncts(std::slice::from_ref(
                            predicate,
                        ))
                        .into_iter()
                        .map(|premise| premises.intern(premise)),
                    );
                    canonicalize_snapshot(&mut arrival.scalar_requirements);
                    for contract in arrival.values.values_mut() {
                        if !payload_predicates::closure_may_change(types, domains, contract)
                        {
                            continue;
                        }
                        payload_predicates::close_with_shared_requirements(
                            types,
                            domains,
                            Arc::make_mut(contract),
                            &arrival.shared_requirements,
                            Some(predicate),
                        );
                    }
                }
                bindings.clear();
                for (actual, parameter) in edge.derivations() {
                    failure = "edge formal is not a whole payload root";
                    if !parameter.path.is_empty() {
                        return None;
                    }
                    if let Some(contract) = arrival
                        .contract(actual.root, edge_predicate)
                        .and_then(|contract| {
                            select_payload_memberships(
                                types,
                                domains,
                                &contract,
                                &actual.path,
                            )
                        })
                    {
                        let Some(contract) = contract.presented(
                            types,
                            domains,
                            *machine.place_types.get(&parameter.root)?,
                        ) else {
                            continue;
                        };
                        bindings.push((parameter.root, contract));
                    }
                }
                for binding in &edge.bindings {
                    let crate::descriptors::descriptor_lifetimes::DescriptorFlowSource::Contents(
                        query,
                    ) = &binding.source
                    else {
                        continue;
                    };
                    let contract = analysis
                        .arrival_payload_targets(identity, current, query)
                        .and_then(|targets| {
                            payload_memberships(types, domains, targets, |root| {
                                arrival.contract(root, edge_predicate)
                            })
                        });
                    if let Some(contract) = contract {
                        failure = "selected edge actual and formal payload carriers differ";
                        let contract = contract.presented(
                            types,
                            domains,
                            *machine.place_types.get(&binding.parameter.root)?,
                        )?;
                        bindings.push((binding.parameter.root, contract));
                    }
                }
                if let Some(predicate) = edge_predicate {
                    for contract in arrival.values.values_mut() {
                        if !contract.scalar_requirements.contains(predicate)
                            && !arrival
                                .shared_requirements
                                .iter()
                                .any(|premise| premise.as_ref() == predicate)
                        {
                            Arc::make_mut(contract)
                                .scalar_requirements
                                .push(predicate.clone());
                        }
                    }
                }
                for parameter in &edge.parameters {
                    arrival.values.remove(&parameter.root);
                }
                for (parameter, contract) in bindings.drain(..) {
                    arrival.values.insert(parameter, contract);
                }
                if edge.target.1 == 0
                    && let Some(parameters) =
                        machine.payload_scalar_parameters.get(&edge.target.0)
                {
                    let incoming_requirements = arrival.scalar_requirements.clone();
                    retire_scalar_instances(&mut arrival, parameters);
                    if let Some(bindings) = machine
                        .payload_scalar_bindings
                        .get(&current)
                        .and_then(|edges| edges.get(ordinal))
                    {
                        let rebound = premises.arrival_requirements(
                            &incoming_requirements,
                            bindings,
                            parameters,
                        );
                        if !contains_rebound_premises(
                            &arrival.scalar_requirements,
                            &rebound,
                        ) {
                            Arc::make_mut(&mut arrival.scalar_requirements).extend(rebound);
                        }
                        canonicalize_snapshot(&mut arrival.scalar_requirements);
                    }
                }
                for contract in arrival.values.values_mut() {
                    if edge_predicate.is_some()
                        && contract
                            .scalar_requirements
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                    {
                        let contract = Arc::make_mut(contract);
                        contract.scalar_requirements.sort();
                        contract.scalar_requirements.dedup();
                    }
                }
                arrival.publish_requirements();
                for contract in arrival.values.values_mut() {
                    if !payload_predicates::closure_may_change(types, domains, contract) {
                        continue;
                    }
                    payload_predicates::close_with_shared_requirements(
                        types,
                        domains,
                        Arc::make_mut(contract),
                        &arrival.shared_requirements,
                        None,
                    );
                }
                let changed = if let Some(snapshot) = snapshots.get_mut(&edge.target) {
                    snapshot.meet(arrival)
                } else {
                    snapshots.insert(edge.target, arrival);
                    true
                };
                if changed && !pending.contains(&edge.target) {
                    pending.push_back(edge.target);
                }
            }
        }
        if let Some(previous) = previous_relevant {
            relevant.merge(previous);
        }
        replay.cached = Some(CachedHistory {
            relevant,
            snapshots,
        });
        failure = "requested payload root has no reached incoming membership history";
        let mut current = match cut {
            PayloadMembershipCut::BeforeOperation
            | PayloadMembershipCut::BeforeOperationWithDeclaredBytes => replay
                .cached
                .as_ref()?
                .snapshots
                .get(&position)?
                .contract(root, None)?,
            PayloadMembershipCut::NormalCompletion => {
                completed.as_ref()?.contract(root, None)?
            }
        };
        if cut != PayloadMembershipCut::BeforeOperationWithDeclaredBytes {
            return Some(current);
        }
        failure = "requested source has no declared payload contract";
        let declared = source(root)?;
        failure = "current and declared source payload carriers differ";
        if current.carrier != declared.carrier {
            return None;
        }
        let byte_domain = |carrier, domain| {
            crate::byte_predicate_carrier(types.iter(), carrier)
                && crate::byte_qualification_predicates_in(
                    domains.iter(),
                    carrier,
                    &[domain],
                )
                .is_some()
        };
        current.qualifications.extend(
            declared
                .qualifications
                .into_iter()
                .filter(|domain| byte_domain(declared.carrier, *domain)),
        );
        current.projected_qualifications.extend(
            declared.projected_qualifications.into_iter().filter(|row| {
                qualification_path_carrier(types.iter(), declared.carrier, &row.path)
                    .is_some_and(|carrier| byte_domain(carrier, row.domain))
            }),
        );
        current.qualifications.sort();
        current.qualifications.dedup();
        current.projected_qualifications.sort();
        current.projected_qualifications.dedup();
        Some(current)
    })();
    result.ok_or(failure)
}

fn contains_rebound_premises(
    retained: &[SharedPremise],
    rebound: &[SharedPremise],
) -> bool {
    if rebound.windows(2).any(|pair| pair[0] > pair[1]) {
        return rebound
            .iter()
            .all(|premise| retained.binary_search(premise).is_ok());
    }
    let mut remaining = retained.iter().peekable();
    rebound
        .iter()
        .all(|premise| next_premise_is(&mut remaining, premise))
}

fn next_premise_is<'a, Premise: PartialOrd + 'a>(
    remaining: &mut std::iter::Peekable<impl Iterator<Item = &'a Premise>>,
    premise: &Premise,
) -> bool {
    while matches!(remaining.peek(), Some(candidate) if *candidate < premise) {
        remaining.next();
    }
    matches!(remaining.peek(), Some(candidate) if *candidate == premise)
}

pub(crate) fn extend_canonical_premises<
    Additional: Borrow<semantic_vocabulary::Proposition>,
>(
    retained: &mut Vec<semantic_vocabulary::Proposition>,
    additional: &[Additional],
) {
    let mut existing = retained.iter().peekable();
    let introduces_premise = additional
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .any(|premise| !next_premise_is(&mut existing, premise));
    if !introduces_premise {
        return;
    }
    let mut combined = Vec::with_capacity(retained.len() + additional.len());
    let mut existing = std::mem::take(retained).into_iter().peekable();
    let mut additional = additional
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    while let (Some(left), Some(right)) = (existing.peek(), additional.peek()) {
        match left.cmp(right) {
            std::cmp::Ordering::Less => combined.extend(existing.next()),
            std::cmp::Ordering::Greater => combined.extend(additional.next().cloned()),
            std::cmp::Ordering::Equal => {
                combined.extend(existing.next());
                additional.next();
            }
        }
    }
    combined.extend(existing);
    combined.extend(additional.cloned());
    *retained = combined;
}

fn retain_common_snapshot(
    retained: &mut Arc<Vec<SharedPremise>>,
    incoming: &[SharedPremise],
) -> bool {
    let mut remaining = incoming.iter().peekable();
    let unchanged = retained
        .iter()
        .all(|premise| next_premise_is(&mut remaining, premise));
    if unchanged {
        return false;
    }
    let mut remaining = incoming.iter().peekable();
    Arc::make_mut(retained).retain(|premise| next_premise_is(&mut remaining, premise));
    true
}

fn canonicalize_snapshot(premises: &mut Arc<Vec<SharedPremise>>) {
    if premises.windows(2).any(|pair| pair[0] >= pair[1]) {
        let premises = Arc::make_mut(premises);
        premises.sort();
        premises.dedup();
    }
}

fn retain_common_premises<
    Retained: Borrow<semantic_vocabulary::Proposition>,
    Incoming: Borrow<semantic_vocabulary::Proposition>,
>(
    retained: &mut Vec<Retained>,
    incoming: &[Incoming],
) -> bool {
    let previous = retained.len();
    let mut remaining = incoming
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    retained.retain(|premise| {
        let premise: &semantic_vocabulary::Proposition = premise.borrow();
        next_premise_is(&mut remaining, premise)
    });
    retained.len() != previous
}

fn retain_captured_premises<
    Local: Borrow<semantic_vocabulary::Proposition>,
    Common: Borrow<semantic_vocabulary::Proposition>,
>(
    incoming: &mut Vec<semantic_vocabulary::Proposition>,
    local: &[Local],
    common: &[Common],
) {
    let mut local = local
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    let mut common = common
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    incoming.retain(|premise| {
        while local.peek().is_some_and(|candidate| *candidate < premise) {
            local.next();
        }
        while common.peek().is_some_and(|candidate| *candidate < premise) {
            common.next();
        }
        local.peek().is_some_and(|candidate| *candidate == premise)
            || common.peek().is_some_and(|candidate| *candidate == premise)
    });
}

fn retain_common_memberships<Membership: Ord>(
    retained: &mut Vec<Membership>,
    incoming: &[Membership],
) {
    if retained.is_empty() {
        return;
    }
    if incoming.is_empty() {
        retained.clear();
        return;
    }
    if retained.as_slice() == incoming {
        return;
    }
    let mut incoming_memberships: Vec<_> = incoming.iter().collect();
    incoming_memberships.sort_unstable();
    retained.retain(|membership| incoming_memberships.binary_search(&membership).is_ok());
}

fn retire(
    content_independent: &[semantic_vocabulary::StructuralDomainId],
    contract: &mut PayloadMembershipContract,
    path: &[StructuralPathSegment],
    scalar_path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) {
    contract.scalar_observations.retain(|observation| {
        !observation.path.starts_with(scalar_path)
            && !scalar_path.starts_with(&observation.path)
    });
    let survives = |domain| content_independent.binary_search(&domain).is_ok();
    // A write beneath an already installed restriction does not issue
    // a new membership. Exact/ancestor replacement still retires it;
    // only the incoming value's checked transfer can supply it again.
    contract
        .qualifications
        .retain(|domain| !path.is_empty() && survives(*domain));
    contract.projected_qualifications.retain(|row| {
        !crate::structural_paths_may_overlap(&row.path, path)
            || (path.len() > row.path.len()
                && terminal_psi::StructuralPathQualification::selected_suffix(
                    &row.path,
                    &path[..row.path.len()],
                )
                .is_some()
                && survives(row.domain))
    });
}

/// Install the value a strong store placed at `path`. The caller has
/// already retired what the old value established there.
fn replace_payload(
    contract: &mut PayloadMembershipContract,
    incoming: PayloadMembershipContract,
    path: &[StructuralPathSegment],
    scalar_path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) {
    if path.is_empty() {
        *contract = incoming;
    } else {
        append_scalar_observations(contract, &incoming, scalar_path);
        terminal_psi::StructuralPathQualification::append_under(
            &incoming.qualifications,
            &incoming.projected_qualifications,
            path,
            &mut contract.projected_qualifications,
        );
    }
}

fn observe_scalar(
    contract: &mut PayloadMembershipContract,
    path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    value: &semantic_vocabulary::ScalarTerm,
) -> Option<semantic_vocabulary::Proposition> {
    if let Some(previous) = contract.scalar_observations.iter_mut().find(|observation| {
        observation.path == path
            && matches!(
                value.scalar_type(),
                semantic_vocabulary::ScalarType::Integer(_)
                    | semantic_vocabulary::ScalarType::Boolean
            )
    }) {
        if let Some(observed) = &previous.value {
            if observed.scalar_type() == value.scalar_type() {
                return (observed != value).then(|| {
                    semantic_vocabulary::Proposition::Equal(value.clone(), observed.clone())
                });
            }
        } else if previous
            .entry
            .as_ref()
            .is_some_and(|entry| entry.scalar_type == value.scalar_type())
        {
            previous.value = Some(value.clone());
            return None;
        }
    }
    contract
        .scalar_observations
        .retain(|observation| observation.path != path);
    contract.scalar_observations.push(PayloadScalarObservation {
        path: path.to_vec(),
        value: Some(value.clone()),
        entry: None,
    });
    None
}

fn clear(state: &mut PayloadContracts) {
    clear_except(state, |_| false)
}

fn clear_except(state: &mut PayloadContracts, preserves: impl Fn(&PlaceId) -> bool) {
    for (place, contract) in &mut state.0 {
        if preserves(place) {
            continue;
        }
        if contract.qualifications.is_empty()
            && contract.projected_qualifications.is_empty()
            && contract.scalar_observations.is_empty()
        {
            continue;
        }
        let contract = Arc::make_mut(contract);
        contract.qualifications.clear();
        contract.projected_qualifications.clear();
        contract.scalar_observations.clear();
    }
}

fn append_scalar_observations(
    destination: &mut PayloadMembershipContract,
    source: &PayloadMembershipContract,
    prefix: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) {
    extend_canonical_premises(
        &mut destination.scalar_requirements,
        &source.scalar_requirements,
    );
    for observation in &source.scalar_observations {
        let mut path = prefix.to_vec();
        path.extend_from_slice(&observation.path);
        destination
            .scalar_observations
            .retain(|previous| previous.path != path);
        destination
            .scalar_observations
            .push(PayloadScalarObservation {
                path,
                value: observation.value.clone(),
                entry: observation.entry.clone(),
            });
    }
}

fn retire_scalar_instances(
    state: &mut State,
    definitions: &[semantic_vocabulary::ValueId],
) {
    if definitions.is_empty() {
        return;
    }
    if state
        .scalar_requirements
        .iter()
        .any(|premise| premise.mentions_any(definitions))
    {
        Arc::make_mut(&mut state.scalar_requirements)
            .retain(|premise| !premise.mentions_any(definitions));
    }
    if state
        .shared_requirements
        .iter()
        .any(|premise| !premise.survives_payload_retirement(definitions))
    {
        Arc::make_mut(&mut state.shared_requirements)
            .retain(|premise| premise.survives_payload_retirement(definitions));
    }
    for contract in state.values.values_mut() {
        if !contract.scalar_observations.iter().any(|observation| {
            (observation.value.is_none() && observation.entry.is_none())
                || observation.value.as_ref().is_some_and(|value| {
                    value.any_value_id(|value| definitions.contains(&value))
                })
        }) && !contract
            .scalar_requirements
            .iter()
            .any(|premise| !survives_payload_retirement(premise, definitions))
        {
            continue;
        }
        let contract = Arc::make_mut(contract);
        contract.scalar_observations.retain_mut(|observation| {
            if observation.value.as_ref().is_some_and(|value| {
                value.any_value_id(|value| definitions.contains(&value))
            }) {
                observation.value = None;
            }
            observation.value.is_some() || observation.entry.is_some()
        });
        retire_payload_premises(&mut contract.scalar_requirements, definitions);
    }
}

fn retire_payload_premises<Premise: Borrow<semantic_vocabulary::Proposition>>(
    premises: &mut Vec<Premise>,
    definitions: &[semantic_vocabulary::ValueId],
) {
    premises.retain(|premise| survives_payload_retirement(premise.borrow(), definitions));
}

fn survives_payload_retirement(
    premise: &semantic_vocabulary::Proposition,
    definitions: &[semantic_vocabulary::ValueId],
) -> bool {
    let mut changed = false;
    let closed = premise.visit_value_ids(|value| {
        changed |= definitions.contains(&value);
    });
    closed && !changed
}

#[cfg(test)]
mod tests;
