//! Memberships describe current payloads, not the storage's arrival value.
//! Stores snapshot their actual source theory; writes retire overlapping rows;
//! reachable control-flow joins intersect evidence. Descriptor replay resolves
//! backing separately. Neither replay grants access, custody or an origin route.

mod entry_snapshots;

use super::contents::select_payload_memberships;
use super::*;
use crate::descriptor_lifetimes::descriptor_path;
use std::collections::VecDeque;

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    values: BTreeMap<PlaceId, PayloadMembershipContract>,
    // Published snapshots keep all premise rosters sorted and unique. An
    // edge join only removes evidence, so ordered intersection is sufficient.
    scalar_requirements: Vec<semantic_vocabulary::Proposition>,
}

// Every transfer works on complete premise rosters. Retained positions factor
// their exact common intersection out of the payloads: copying the same growing
// theory into every SSA payload at every position otherwise multiplies memory
// by both the number of values and the length of the control-flow prefix.
// Do not assume the state's ambient premises occur in every payload (notably at
// entry). Compute the actual intersection, then restore it before any consumer.
struct Snapshot {
    state: State,
    common_requirements: Vec<semantic_vocabulary::Proposition>,
}

impl Snapshot {
    fn capture(mut state: State) -> Self {
        let mut contracts = state.values.values();
        let mut common_requirements = contracts
            .next()
            .map(|contract| contract.scalar_requirements.clone())
            .unwrap_or_default();
        for contract in contracts {
            retain_common_premises(&mut common_requirements, &contract.scalar_requirements);
        }
        for contract in state.values.values_mut() {
            let mut common = common_requirements.iter().peekable();
            contract.scalar_requirements.retain(|premise| {
                while common.peek().is_some_and(|candidate| *candidate < premise) {
                    common.next();
                }
                !common.peek().is_some_and(|candidate| *candidate == premise)
            });
            contract.scalar_requirements.shrink_to_fit();
        }
        Self {
            state,
            common_requirements,
        }
    }

    fn expand(&self) -> State {
        let mut state = self.state.clone();
        for contract in state.values.values_mut() {
            self.restore(contract);
        }
        state
    }

    fn contract(&self, root: PlaceId) -> Option<PayloadMembershipContract> {
        let mut contract = self.state.values.get(&root)?.clone();
        self.restore(&mut contract);
        Some(contract)
    }

    fn restore(&self, contract: &mut PayloadMembershipContract) {
        extend_canonical_premises(&mut contract.scalar_requirements, &self.common_requirements);
    }
}

struct CachedHistory {
    relevant: super::payload_reachability::Reachability,
    snapshots: BTreeMap<DescriptorPosition, Snapshot>,
}

/// Exact declaration data in one current executable representation. Prepared
/// rows are data, not predicates or callbacks; the history still derives every
/// value observation from the supplied current graph and actual operations.
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

/// Query-local reuse bound to one immutable Terminal machine and its exact
/// catalog/descriptor graph. A producer creates this inside its read sweep and
/// drops it before publishing changed rosters. Arbitrary source callbacks are
/// deliberately excluded from this reusable interface.
pub struct PayloadMembershipHistory<'input> {
    types: &'input [StructuralTypeDeclaration],
    domains: &'input [terminal_psi::StructuralDomainDeclaration],
    graph: &'input [DescriptorMachine],
    sources: PayloadSources<'input>,
    cached: Option<CachedHistory>,
    /// Materialized only for an exact scalar-snapshot query. Ordinary numeric
    /// membership sweeps keep their existing input and allocation profile.
    entry_snapshots: Option<Vec<(PlaceId, Vec<super::PayloadScalarObservation>)>>,
}
impl<'input> PayloadMembershipHistory<'input> {
    pub fn new(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        machine: &'input TerminalMachine,
    ) -> Self {
        Self {
            types,
            domains,
            graph,
            sources: PayloadSources::Terminal(machine),
            cached: None,
            entry_snapshots: None,
        }
    }
    /// Reuse this same history over normalized current-body declarations.
    /// Missing declarations stay unavailable when queried; an absent contract
    /// never becomes an established empty payload or initialized local.
    pub fn for_current_contracts(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
    ) -> Result<Self, &'static str> {
        let current =
            descriptor_machine(graph, identity).ok_or("current payload machine is absent")?;
        for (index, (root, contract)) in contracts.iter().enumerate() {
            if current.place_types.get(root) != Some(&contract.carrier)
                || contracts[..index]
                    .iter()
                    .any(|(previous, _)| previous == root)
            {
                return Err("current payload declaration disagrees with its exact graph carrier");
            }
        }
        Ok(Self {
            types,
            domains,
            graph,
            sources: PayloadSources::Current {
                identity,
                parameters,
                contracts,
            },
            cached: None,
            entry_snapshots: None,
        })
    }
    pub fn current_selected_payload_contract(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        let identity = self.sources.identity();
        let sources = &self.sources;
        let source = |place| sources.contract(place);
        let root_contract = source(root).ok_or("selected payload root is absent")?;
        let query = descriptor_contents_query(self.types, root, root_contract.carrier, path, None)
            .ok_or("selected payload path has no typed coordinate")?;
        let mut analysis = DescriptorAnalysis::new(self.types, self.graph);
        let targets = analysis
            .payload_targets(identity, position, &query)
            .ok_or("selected payload backing is unresolved")?;
        let mut failure = None;
        let entry_snapshots = self.entry_snapshots.as_deref().unwrap_or(&[]);
        let cached = &mut self.cached;
        let contract = payload_memberships(self.types, self.domains, targets, |place| {
            current_payload_contract_reusing(
                self.types,
                self.domains,
                self.graph,
                identity,
                position,
                place,
                &source,
                entry_snapshots,
                cached,
            )
            .map_err(|reason| failure = Some(reason))
            .ok()
        });
        contract.ok_or(failure.unwrap_or("selected payload memberships are unresolved"))
    }

    /// Observe this owned value occurrence before the operation, without
    /// following a descriptor to its backing. Mutable fields use their current
    /// contract after preceding stores, not the producer's establishment roster.
    pub fn current_payload_contract(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
    ) -> Result<PayloadMembershipContract, &'static str> {
        let identity = self.sources.identity();
        let sources = &self.sources;
        current_payload_contract_reusing(
            self.types,
            self.domains,
            self.graph,
            identity,
            position,
            root,
            &|place| sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.cached,
        )
    }
    /// Observe a primitive value snapshot, without asserting a predicate about
    /// it. Unknown entry contents acquire only a typed entry identity; existing
    /// history transports that identity through actual copies and intersects it
    /// across every predecessor. No synthetic SSA or executable read is added.
    pub fn scalar_snapshot(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[crate::descriptor_lifetimes::DescriptorPathSegment],
    ) -> Result<super::PayloadScalarObservation, &'static str> {
        if self.entry_snapshots.is_none() {
            self.entry_snapshots = Some(entry_snapshots::collect(
                self.types,
                self.sources.parameters(),
            ));
            self.cached = None;
        }
        let contract = self.current_payload_contract(position, root)?;
        let expected =
            super::payload_predicates::observation_type(self.types, contract.carrier, path)
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

/// Observe the current payload selected by an operand, following descriptors
/// to every possible backing. This proves facts about the selected bytes; it
/// grants neither a copy nor a loan and never reads the receiving contract.
pub fn current_selected_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    path: &[StructuralPathSegment],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let root_contract = source(root).ok_or("selected payload root is absent")?;
    let query = descriptor_contents_query(types, root, root_contract.carrier, path, None)
        .ok_or("selected payload path has no typed coordinate")?;
    let mut analysis = DescriptorAnalysis::new(types, graph);
    let targets = analysis
        .payload_targets(identity, position, &query)
        .ok_or("selected payload backing is unresolved")?;
    let mut history_failure = None;
    let contract = payload_memberships(types, domains, targets, |place| {
        current_payload_contract(types, domains, graph, identity, position, place, source)
            .map_err(|reason| history_failure = Some(reason))
            .ok()
    });
    contract.ok_or(history_failure.unwrap_or("selected payload memberships are unresolved"))
}

/// Reconstruct memberships at an exact pre-operation position. Entry supplies
/// only actual input contracts; producer rosters enter at establishment. A
/// receiving store or requested copy annotation never seeds the fixed point.
/// The bounded worklist fails closed if the current graph cannot be resolved.
pub fn current_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    current_payload_contract_reusing(
        types,
        domains,
        graph,
        identity,
        position,
        root,
        source,
        &[],
        &mut None,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PayloadMembershipCut {
    BeforeOperation,
    NormalCompletion,
}

/// Memberships after this operation's effects and normal postconditions, before
/// successor bindings. The caller captures an input's pointer identity before
/// moves, then asks about that backing at this cut. This never reads the new
/// result's annotation as evidence for an input fact. Borrowed-storage replay
/// retains pointer-reference and view contents through the same typed graph.
pub(crate) fn completed_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    payload_contract_at_cut(
        types,
        domains,
        graph,
        identity,
        position,
        root,
        source,
        &[],
        &mut None,
        PayloadMembershipCut::NormalCompletion,
    )
}

fn current_payload_contract_reusing(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<super::PayloadScalarObservation>)],
    cached: &mut Option<CachedHistory>,
) -> Result<PayloadMembershipContract, &'static str> {
    payload_contract_at_cut(
        types,
        domains,
        graph,
        identity,
        position,
        root,
        source,
        entry_snapshots,
        cached,
        PayloadMembershipCut::BeforeOperation,
    )
}

#[allow(clippy::too_many_arguments)]
fn payload_contract_at_cut(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<super::PayloadScalarObservation>)],
    cached: &mut Option<CachedHistory>,
    cut: PayloadMembershipCut,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut failure = "descriptor machine is absent";
    let result = (|| {
        let machine = descriptor_machine(graph, identity)?;
        failure = "payload query reachability is invalid or exceeds its finite budget";
        let relevant = super::payload_reachability::predecessors(machine, position)?;
        let previous = cached.take();
        let frontier = previous
            .as_ref()
            .and_then(|previous| relevant.extension_frontier(&previous.relevant, &machine.flow));
        let (mut snapshots, mut pending) = if let Some(frontier) = frontier {
            (
                previous
                    .expect("frontier owns the preceding immutable history")
                    .snapshots,
                frontier,
            )
        } else {
            // Release an incompatible prefix before constructing its replacement.
            drop(previous);
            let mut initial = State::default();
            if let Some((_, requirements)) = &machine.payload_entry_predicates {
                initial.scalar_requirements =
                    super::scalar_contracts::scalar_conjuncts(requirements);
            }
            initial.scalar_requirements.sort();
            initial.scalar_requirements.dedup();
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
                BTreeMap::from([(machine.entry, Snapshot::capture(initial))]),
                VecDeque::from([machine.entry]),
            )
        };
        let mut analysis = match cut {
            PayloadMembershipCut::BeforeOperation => DescriptorAnalysis::new(types, graph),
            PayloadMembershipCut::NormalCompletion => {
                DescriptorAnalysis::with_borrowed_storage_slots(types, graph)
            }
        };
        let mut completed = None;
        let mut remaining = 65_536usize;
        while let Some(current) = pending.pop_front() {
            failure = "payload history worklist exceeded its finite budget";
            remaining = remaining.checked_sub(1)?;
            if !relevant.contains(current) {
                continue;
            }
            failure = "payload history flow position is absent";
            let mut before = snapshots.get(&current)?.expand();
            if let Some(definitions) = machine.payload_scalar_definitions.get(&current) {
                retire_scalar_instances(&mut before, definitions);
            }
            let step = machine.flow.get(&current)?;
            // Only derivation actuals read the pre-write payloads below. Keep
            // one immutable copy of each such root, not a second full state.
            // Selectors and fallible backing checks still run in their original
            // order, so simultaneous stores retain both snapshot semantics and
            // diagnostic precedence. Unused roots move directly into `after`.
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
                    source_values
                        .entry(actual.root)
                        .or_insert_with(|| contract.clone());
                }
            }
            let mut after = before;
            if let Some(writes) = machine.payload_invalidations.get(&current) {
                for query in writes {
                    let targets = query
                        .as_ref()
                        .and_then(|query| analysis.payload_targets(identity, current, query));
                    match targets {
                        Some(targets) => {
                            for target in targets {
                                match target {
                                    DescriptorPayloadTarget::Storage(location) => {
                                        if let Some(contract) = after.values.get_mut(&location.root)
                                        {
                                            failure = "payload write path has no typed coordinate";
                                            let path = descriptor_path(
                                                types,
                                                contract.carrier,
                                                &location.path,
                                            )?;
                                            retire(contract, &path, &location.path);
                                        }
                                    }
                                    DescriptorPayloadTarget::Incoming(_) => {
                                        clear(&mut after.values)
                                    }
                                }
                            }
                        }
                        None => clear(&mut after.values),
                    }
                }
            }
            // Snapshot all RHSs before applying any destination update. A repeated
            // or overlapping destination cannot acquire the last writer's theory.
            let mut stores = Vec::new();
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
                let incoming = source_values.get(&actual.root).and_then(|contract| {
                    select_payload_memberships(types, domains, contract, &actual.path)
                });
                failure = "structural store backing is unresolved";
                let destinations = analysis.storage_origins(identity, current, destination)?;
                let strong = destinations.len() == 1;
                for destination in destinations {
                    stores.push((destination, incoming.clone(), strong));
                }
            }
            // Decide simultaneous overlap before consuming the snapshots. The
            // exact-carrier path can then move its theory without cloning it.
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
            for (destination, incoming, strong) in stores {
                let Some(contract) = after.values.get_mut(&destination.root) else {
                    continue;
                };
                failure = "structural store path has no typed coordinate";
                let path = descriptor_path(types, contract.carrier, &destination.path)?;
                retire(contract, &path, &destination.path);
                if !strong {
                    continue;
                }
                let Some(incoming) = incoming else {
                    continue;
                };
                failure = "structural store carrier does not match its actual source";
                let destination_carrier =
                    crate::qualification_path_carrier(types.iter(), contract.carrier, &path)?;
                let incoming = incoming.presented(types, domains, destination_carrier)?;
                if path.is_empty() {
                    *contract = incoming;
                } else {
                    append_scalar_observations(contract, &incoming, &destination.path);
                    terminal_psi::StructuralPathQualification::append_under(
                        &incoming.qualifications,
                        &incoming.projected_qualifications,
                        &path,
                        &mut contract.projected_qualifications,
                    );
                }
            }
            if let Some(producer) = machine.payload_producers.get(&current) {
                failure = "established payload contract is absent";
                let mut produced = source(*producer)?;
                // Source callbacks need not order their premises. Normalize at
                // this boundary once; child composition and stores preserve it.
                produced.scalar_requirements.sort();
                produced.scalar_requirements.dedup();
                // Constructors and copies snapshot their actual children before
                // consuming them. A result annotation supplies no scalar content.
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
                    if let Some(incoming) = source_values.get(&actual.root).and_then(|contract| {
                        select_payload_memberships(types, domains, contract, &actual.path)
                    }) {
                        let path = descriptor_path(types, produced.carrier, &destination.path)?;
                        let destination_carrier = crate::qualification_path_carrier(
                            types.iter(),
                            produced.carrier,
                            &path,
                        )?;
                        let Some(incoming) =
                            incoming.presented(types, domains, destination_carrier)
                        else {
                            continue;
                        };
                        append_scalar_observations(&mut produced, &incoming, &destination.path);
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
                    // New result storage has no pre-operation backing. Every
                    // other observation, including a borrowed call actual,
                    // must resolve to one exact current storage coordinate.
                    let resolved;
                    let destination =
                        if machine.payload_producers.get(&current) == Some(&destination.root) {
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
                    if super::payload_predicates::observation_type(
                        types,
                        contract.carrier,
                        &destination.path,
                    ) != Some(value.scalar_type())
                    {
                        continue;
                    }
                    contract.scalar_observations.retain(|observation| {
                        !observation.path.starts_with(&destination.path)
                            && !destination.path.starts_with(&observation.path)
                    });
                    contract
                        .scalar_observations
                        .push(super::PayloadScalarObservation {
                            path: destination.path.clone(),
                            value: Some(value.clone()),
                            entry: None,
                        });
                }
            }
            if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                for (location, value) in reads {
                    if let Some(contract) = after.values.get_mut(&location.root) {
                        if let Some(equality) = observe_scalar(contract, &location.path, value) {
                            after.scalar_requirements.push(equality);
                        }
                    }
                }
            }
            if let Some(requirements) = machine.payload_scalar_postconditions.get(&current) {
                after
                    .scalar_requirements
                    .extend(requirements.iter().cloned());
            }
            // Reads can add equations even when this step has no postcondition.
            // Normalize at publication, after all local append operations.
            after.scalar_requirements.sort();
            after.scalar_requirements.dedup();
            for contract in after.values.values_mut() {
                // Snapshot restoration, retirement, selected stores and the
                // normalized producer preserve canonical payload rosters. Do
                // not rescan every unchanged payload with sort/dedup per step.
                extend_canonical_premises(
                    &mut contract.scalar_requirements,
                    &after.scalar_requirements,
                );
                super::payload_predicates::close(types, domains, contract);
            }
            if cut == PayloadMembershipCut::NormalCompletion && current == position {
                // The worklist may revisit a loop cut. Replace this observation
                // only after all incoming rows have met and this transfer has
                // run again; the final value is the fixed-point completion.
                completed = Some(Snapshot::capture(after.clone()));
            }
            // Only distinct arrivals need independent state. Move the completed
            // transfer into the last relevant edge instead of cloning every
            // payload's full scalar theory even on a straight-line step.
            let mut successors = step
                .successors
                .iter()
                .enumerate()
                .filter(|(ordinal, _)| relevant.includes_edge(current, *ordinal))
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
                    arrival
                        .scalar_requirements
                        .extend(super::scalar_contracts::scalar_conjuncts(
                            std::slice::from_ref(predicate),
                        ));
                    arrival.scalar_requirements.sort();
                    arrival.scalar_requirements.dedup();
                    for contract in arrival.values.values_mut() {
                        if !contract.scalar_requirements.contains(predicate) {
                            contract.scalar_requirements.push(predicate.clone());
                        }
                        super::payload_predicates::close(types, domains, contract);
                    }
                }
                // Snapshot only actual binding results before retiring any
                // formal. Loop edges can read and rebind the same root, so the
                // entire binding batch must observe the pre-arrival state.
                // The other payloads need no second full-state snapshot.
                let mut bindings = Vec::new();
                for (actual, parameter) in edge.derivations() {
                    failure = "edge formal is not a whole payload root";
                    if !parameter.path.is_empty() {
                        return None;
                    }
                    if let Some(contract) = arrival.values.get(&actual.root).and_then(|contract| {
                        select_payload_memberships(types, domains, contract, &actual.path)
                    }) {
                        // Exact descriptor bindings also carry reference access
                        // conversions. Memberships are carrier-specific: an
                        // incompatible row supplies no evidence for that formal,
                        // but cannot invalidate other payloads on the edge.
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
                // A selected contents family supplies only facts justified by
                // every candidate; its shared typed row never claims an address.
                for binding in &edge.bindings {
                    let crate::descriptor_lifetimes::DescriptorFlowSource::Contents(query) =
                        &binding.source
                    else {
                        continue;
                    };
                    let contract = analysis
                        .arrival_payload_targets(identity, current, query)
                        .and_then(|targets| {
                            payload_memberships(types, domains, targets, |root| {
                                arrival.values.get(&root).cloned()
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
                for parameter in &edge.parameters {
                    arrival.values.remove(&parameter.root);
                }
                for (parameter, contract) in bindings {
                    arrival.values.insert(parameter, contract);
                }
                // Only an actual block arrival rebinds its formals. Ordinary
                // operation steps within the block keep that same SSA instance.
                if edge.target.1 == 0
                    && let Some(parameters) = machine.payload_scalar_parameters.get(&edge.target.0)
                {
                    let incoming_requirements = arrival.scalar_requirements.clone();
                    retire_scalar_instances(&mut arrival, parameters);
                    if let Some(bindings) = machine
                        .payload_scalar_bindings
                        .get(&current)
                        .and_then(|edges| edges.get(ordinal))
                    {
                        arrival.scalar_requirements.extend(
                            super::scalar_contracts::arrival_requirements(
                                &incoming_requirements,
                                bindings,
                                parameters,
                            ),
                        );
                        arrival.scalar_requirements.sort();
                        arrival.scalar_requirements.dedup();
                    }
                }
                for contract in arrival.values.values_mut() {
                    // Only the edge predicate can append to a payload roster
                    // since the canonical post-step publication. Selection,
                    // presentation, intersection and retirement preserve its
                    // order. Keep predicate normalization after the existing
                    // intermediate closure checks, but do not rescan unchanged
                    // rosters on ordinary edges.
                    if edge_predicate.is_some() {
                        contract.scalar_requirements.sort();
                        contract.scalar_requirements.dedup();
                    }
                    extend_canonical_premises(
                        &mut contract.scalar_requirements,
                        &arrival.scalar_requirements,
                    );
                    super::payload_predicates::close(types, domains, contract);
                }
                let changed = if let Some(snapshot) = snapshots.get_mut(&edge.target) {
                    let mut existing = snapshot.expand();
                    let mut changed = retain_common_premises(
                        &mut existing.scalar_requirements,
                        &arrival.scalar_requirements,
                    );
                    let previous_values = existing.values.len();
                    existing.values.retain(|place, contract| {
                        let Some(incoming) = arrival.values.get(place) else {
                            return false;
                        };
                        if contract.carrier != incoming.carrier {
                            return false;
                        }
                        changed |= retain_common_premises(
                            &mut contract.scalar_requirements,
                            &incoming.scalar_requirements,
                        );
                        let previous_counts = (
                            contract.scalar_observations.len(),
                            contract.qualifications.len(),
                            contract.projected_qualifications.len(),
                        );
                        changed |= super::PayloadScalarObservation::intersect(
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
                                contract.scalar_observations.len(),
                                contract.qualifications.len(),
                                contract.projected_qualifications.len(),
                            );
                        true
                    });
                    // Membership rosters only remove rows. Scalar observations
                    // additionally meet their expression and entry components;
                    // their changed flag above covers equal-cardinality loss.
                    let changed = changed || existing.values.len() != previous_values;
                    *snapshot = Snapshot::capture(existing);
                    changed
                } else {
                    snapshots.insert(edge.target, Snapshot::capture(arrival));
                    true
                };
                if changed && !pending.contains(&edge.target) {
                    pending.push_back(edge.target);
                }
            }
        }
        *cached = Some(CachedHistory {
            relevant,
            snapshots,
        });
        failure = "requested payload root has no reached incoming membership history";
        let mut current = match cut {
            PayloadMembershipCut::BeforeOperation => {
                cached.as_ref()?.snapshots.get(&position)?.contract(root)?
            }
            PayloadMembershipCut::NormalCompletion => completed.as_ref()?.contract(root)?,
        };
        if cut == PayloadMembershipCut::NormalCompletion {
            // The completed state already applied writes and closed its actual
            // postconditions. Do not re-seed it from an earlier declaration.
            return Some(current);
        }
        failure = "requested source has no declared payload contract";
        let declared = source(root)?;
        failure = "current and declared source payload carriers differ";
        if current.carrier != declared.carrier {
            return None;
        }
        // Pure byte predicates in an actual source's retained contract are
        // standing validity obligations. Independent arrival, primitive-write,
        // replacement and writable-call checks preserve that floor. History owns
        // additional transient/routed memberships; the receiving copy contributes
        // none. This query must never discharge those independent checks itself.
        let byte_domain = |carrier, domain| {
            crate::byte_predicate_carrier(types.iter(), carrier)
                && crate::byte_qualification_predicates_in(domains.iter(), carrier, &[domain])
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
                crate::qualification_path_carrier(types.iter(), declared.carrier, &row.path)
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

/// Union two sorted, unique premise rosters without cloning and then discarding
/// every ambient premise already present in each live payload. Callers normalize
/// newly produced or rebound contracts before this operation; retained snapshots
/// already satisfy that invariant. Preserve the existing proposition on equality,
/// just as stable append/sort/dedup did. No new proposition is established here.
fn extend_canonical_premises(
    retained: &mut Vec<semantic_vocabulary::Proposition>,
    additional: &[semantic_vocabulary::Proposition],
) {
    let mut existing = retained.iter().peekable();
    let introduces_premise = additional.iter().any(|premise| {
        while existing
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            existing.next();
        }
        !existing
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    });
    if !introduces_premise {
        return;
    }
    let mut combined = Vec::with_capacity(retained.len() + additional.len());
    let mut existing = std::mem::take(retained).into_iter().peekable();
    let mut additional = additional.iter().peekable();
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

/// Intersect canonical premise rosters without searching the whole arrival for
/// every retained premise. The caller publishes sorted, unique snapshots.
fn retain_common_premises(
    retained: &mut Vec<semantic_vocabulary::Proposition>,
    incoming: &[semantic_vocabulary::Proposition],
) -> bool {
    let previous = retained.len();
    let mut remaining = incoming.iter().peekable();
    retained.retain(|premise| {
        while remaining
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            remaining.next();
        }
        remaining
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    });
    retained.len() != previous
}

/// A large qualified aggregate can carry thousands of projected memberships.
/// Searching its complete incoming roster for every retained row makes every
/// join quadratic. Source callbacks need not publish sorted rosters, so sort
/// borrowed references once and leave both contracts' order and duplicates
/// untouched. This computes exactly the old membership filter, not a stronger
/// meet or a new canonicalization requirement on callers.
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
    let mut incoming_memberships: Vec<_> = incoming.iter().collect();
    incoming_memberships.sort_unstable();
    retained.retain(|membership| incoming_memberships.binary_search(&membership).is_ok());
}

fn retire(
    contract: &mut PayloadMembershipContract,
    path: &[StructuralPathSegment],
    scalar_path: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) {
    contract.scalar_observations.retain(|observation| {
        !observation.path.starts_with(scalar_path) && !scalar_path.starts_with(&observation.path)
    });
    contract.qualifications.clear();
    contract
        .projected_qualifications
        .retain(|row| !crate::structural_paths_may_overlap(&row.path, path));
}

/// An unchanged location still contains its earlier observed value. Retain
/// that common observation across branch-local rereads and connect the new SSA
/// result to it. Writes and dynamic-instance retirement remove observations
/// before this operation, so neither can resurrect a stale field equation.
fn observe_scalar(
    contract: &mut PayloadMembershipContract,
    path: &[crate::descriptor_lifetimes::DescriptorPathSegment],
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
    contract
        .scalar_observations
        .push(super::PayloadScalarObservation {
            path: path.to_vec(),
            value: Some(value.clone()),
            entry: None,
        });
    None
}

fn clear(state: &mut BTreeMap<PlaceId, PayloadMembershipContract>) {
    for contract in state.values_mut() {
        contract.qualifications.clear();
        contract.projected_qualifications.clear();
        contract.scalar_observations.clear();
    }
}

fn append_scalar_observations(
    destination: &mut PayloadMembershipContract,
    source: &PayloadMembershipContract,
    prefix: &[crate::descriptor_lifetimes::DescriptorPathSegment],
) {
    // Both sides have canonical premises: actual selection and presentation
    // preserve snapshots, and a fresh destination is normalized at its source
    // callback boundary. Composition needs neither repeated normalization nor
    // a quadratic per-premise membership search.
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
            .push(super::PayloadScalarObservation {
                path,
                value: observation.value.clone(),
                entry: observation.entry.clone(),
            });
    }
}

/// SSA names acquire new dynamic instances on repeated execution. Completed
/// payload memberships survive; observations and premises naming old instances
/// must not classify newly read values or rebound block formals.
fn retire_scalar_instances(state: &mut State, definitions: &[semantic_vocabulary::ValueId]) {
    if definitions.is_empty() {
        return;
    }
    state
        .scalar_requirements
        .retain(|premise| !premise.any_value_id(|value| definitions.contains(&value)));
    for contract in state.values.values_mut() {
        contract.scalar_observations.retain_mut(|observation| {
            if observation
                .value
                .as_ref()
                .is_some_and(|value| value.any_value_id(|value| definitions.contains(&value)))
            {
                observation.value = None;
            }
            observation.value.is_some() || observation.entry.is_some()
        });
        contract.scalar_requirements.retain(|premise| {
            let mut changed = false;
            let closed = premise.visit_value_ids(|value| {
                changed |= definitions.contains(&value);
            });
            closed && !changed
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor_lifetimes::DescriptorPathSegment;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
    };

    #[test]
    fn rereads_preserve_common_observation_but_writes_and_instances_retire_it() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let value = |identity| {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer),
            )
        };
        let path = vec![DescriptorPathSegment::FixedIndex(0)];
        let mut contract = PayloadMembershipContract {
            carrier: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            scalar_observations: Vec::new(),
            scalar_requirements: Vec::new(),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        assert_eq!(observe_scalar(&mut contract, &path, &value(1)), None);
        let mut other_branch = contract.clone();
        assert_eq!(
            observe_scalar(&mut contract, &path, &value(2)),
            Some(Proposition::Equal(value(2), value(1)))
        );
        assert_eq!(
            observe_scalar(&mut other_branch, &path, &value(3)),
            Some(Proposition::Equal(value(3), value(1)))
        );
        assert_eq!(
            contract.scalar_observations,
            other_branch.scalar_observations
        );
        retire(
            &mut contract,
            &[StructuralPathSegment::FixedIndex(0)],
            &path,
        );
        assert_eq!(observe_scalar(&mut contract, &path, &value(4)), None);
        assert_eq!(contract.scalar_observations[0].value, Some(value(4)));
        let root = PlaceId::new(1).unwrap();
        let mut state = State {
            values: BTreeMap::from([(root, other_branch)]),
            scalar_requirements: Vec::new(),
        };
        retire_scalar_instances(&mut state, &[ValueId::new(1).unwrap()]);
        assert_eq!(
            observe_scalar(state.values.get_mut(&root).unwrap(), &path, &value(5)),
            None
        );
    }
}
