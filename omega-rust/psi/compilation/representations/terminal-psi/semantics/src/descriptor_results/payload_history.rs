//! Memberships describe current payloads, not the storage's arrival value.
//! Stores snapshot their actual source theory; writes retire overlapping rows;
//! reachable control-flow joins intersect evidence. Descriptor replay resolves
//! backing separately. Neither replay grants access, custody or an origin route.

use super::contents::select_payload_memberships;
use super::*;
use crate::descriptor_lifetimes::descriptor_path;
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    values: BTreeMap<PlaceId, PayloadMembershipContract>,
    // Published snapshots keep all premise rosters sorted and unique. An
    // edge join only removes evidence, so ordered intersection is sufficient.
    scalar_requirements: Arc<Vec<Arc<semantic_vocabulary::Proposition>>>,
    // Published payloads contain their local roster plus this shared roster.
    // Shared premises are always a subset of the ambient premises above.
    // Entry starts with none: an entry assumption need not occur in an input
    // payload's own contract. Ordinary publication attaches ambient premises
    // to every payload, so transfers need not expand that theory per SSA value.
    // Successive pre-operation snapshots usually keep the same complete theory.
    // Sharing only across payloads still copied its expression trees twice per
    // position. Keep the published roster immutable across positions as well;
    // writers detach at additions, removals or necessary normalization. Ambient
    // and payload retirement differ even when their input allocation agrees.
    // Each immutable proposition is shared too: detaching a changed roster
    // copies its handles, rather than every retained expression tree. Ordering
    // and equality still compare the complete proposition values.
    shared_requirements: Arc<Vec<Arc<semantic_vocabulary::Proposition>>>,
}

impl State {
    /// Publish only after the transfer has normalized its ambient/local rows.
    /// Previous shared rows survive in the new ambient theory: retirement uses
    /// the payload filter on shared rows, and joins intersect both theories.
    /// Newly produced or rebound payloads remain raw until this publication;
    /// their transfer suffix has no premise reader before this point.
    fn publish_requirements(&mut self) {
        for contract in self.values.values_mut() {
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
            contract.scalar_requirements.shrink_to_fit();
        }
        self.shared_requirements = self.scalar_requirements.clone();
    }

    /// Materialize only an actual reader's complete theory. A predicate edge
    /// appends its whole predicate after the ordered local/shared union, just
    /// as the former eager expansion did before final edge normalization.
    fn contract(
        &self,
        root: PlaceId,
        predicate: Option<&semantic_vocabulary::Proposition>,
    ) -> Option<PayloadMembershipContract> {
        let mut contract = self.values.get(&root)?.clone();
        extend_canonical_premises(&mut contract.scalar_requirements, &self.shared_requirements);
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
        let mut changed =
            retain_common_snapshot(&mut self.scalar_requirements, &arrival.scalar_requirements);
        let previous_values = self.values.len();
        self.values.retain(|place, contract| {
            let Some(mut incoming) = arrival.values.remove(place) else {
                return false;
            };
            if contract.carrier != incoming.carrier {
                return false;
            }
            let previous_premises = contract.scalar_requirements.len() + previous_shared.len();
            // Keep every cross term of (old local + old shared) intersected
            // with (incoming local + incoming shared). A fact can be shared
            // on one path and local on the other. Only shared/shared becomes
            // the new shared roster; the other terms remain payload-local.
            retain_captured_premises(
                &mut contract.scalar_requirements,
                &incoming.scalar_requirements,
                &arrival.shared_requirements,
            );
            retain_common_premises(&mut incoming.scalar_requirements, &previous_shared);
            extend_canonical_premises(
                &mut contract.scalar_requirements,
                &incoming.scalar_requirements,
            );
            changed |= contract.scalar_requirements.len() + shared.len() != previous_premises;
            let previous_counts = (
                contract.scalar_observations.len(),
                contract.qualifications.len(),
                contract.projected_qualifications.len(),
            );
            contract
                .scalar_observations
                .retain(|observation| incoming.scalar_observations.contains(observation));
            retain_common_memberships(&mut contract.qualifications, &incoming.qualifications);
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
        self.shared_requirements = shared;
        // All logical changes are losses. Moving a fact between local and
        // shared storage alone must not enqueue another worklist iteration.
        changed || self.values.len() != previous_values
    }
}

struct CachedHistory {
    relevant: super::payload_reachability::Reachability,
    snapshots: BTreeMap<DescriptorPosition, State>,
}

/// Query-local reuse bound to one immutable Terminal machine and its exact
/// catalog/descriptor graph. A producer creates this inside its read sweep and
/// drops it before publishing changed rosters. Arbitrary source callbacks are
/// deliberately excluded from this reusable interface.
pub struct PayloadMembershipHistory<'input> {
    types: &'input [StructuralTypeDeclaration],
    domains: &'input [terminal_psi::StructuralDomainDeclaration],
    graph: &'input [DescriptorMachine],
    descriptor: Option<&'input DescriptorMachine>,
    machine: &'input TerminalMachine,
    incoming: Option<Option<super::payload_reachability::IncomingFlow>>,
    // Every query prepares this same machine's immutable pre-operation replay.
    // Share it with payload transfer instead of retaining two complete replays.
    analysis: Option<DescriptorAnalysis<'input>>,
    cached: Option<CachedHistory>,
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
            descriptor: descriptor_machine(graph, machine.id),
            machine,
            incoming: None,
            analysis: None,
            cached: None,
        }
    }
    pub fn current_selected_payload_contract(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        let types = self.types;
        let graph = self.graph;
        let mut analysis = self
            .analysis
            .take()
            .unwrap_or_else(|| DescriptorAnalysis::new(types, graph));
        let result = self.current_selected_payload_contract_in(&mut analysis, position, root, path);
        self.analysis = Some(analysis);
        result
    }
    pub fn current_selected_payload_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        let machine = self.machine;
        let source = |place| super::terminal_payload_contract(machine, place);
        let root_contract = source(root).ok_or("selected payload root is absent")?;
        let query = descriptor_contents_query(self.types, root, root_contract.carrier, path, None)
            .ok_or("selected payload path has no typed coordinate")?;
        let targets = analysis
            .payload_targets(machine.id, position, &query)
            .ok_or("selected payload backing is unresolved")?;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| {
                descriptor.and_then(|machine| super::payload_reachability::incoming_flow(&machine.flow))
            })
            .as_ref();
        let mut failure = None;
        let cached = &mut self.cached;
        let contract = payload_memberships(self.types, self.domains, targets, |place| {
            current_payload_contract_reusing(
                self.types,
                self.domains,
                descriptor,
                incoming,
                analysis,
                position,
                place,
                &source,
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
        let types = self.types;
        let graph = self.graph;
        let mut analysis = self
            .analysis
            .take()
            .unwrap_or_else(|| DescriptorAnalysis::new(types, graph));
        let result = self.current_payload_contract_in(&mut analysis, position, root);
        self.analysis = Some(analysis);
        result
    }
    pub fn current_payload_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
    ) -> Result<PayloadMembershipContract, &'static str> {
        let machine = self.machine;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| {
                descriptor.and_then(|machine| super::payload_reachability::incoming_flow(&machine.flow))
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
            &|place| super::terminal_payload_contract(machine, place),
            &mut self.cached,
        )
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
    let descriptor = descriptor_machine(graph, identity);
    let incoming =
        descriptor.and_then(|machine| super::payload_reachability::incoming_flow(&machine.flow));
    let mut cached = None;
    let mut history_failure = None;
    let contract = payload_memberships(types, domains, targets, |place| {
        current_payload_contract_reusing(
            types,
            domains,
            descriptor,
            incoming.as_ref(),
            &mut analysis,
            position,
            place,
            source,
            &mut cached,
        )
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
    let mut analysis = DescriptorAnalysis::new(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming =
        descriptor.and_then(|machine| super::payload_reachability::incoming_flow(&machine.flow));
    current_payload_contract_reusing(
        types,
        domains,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        source,
        &mut None,
    )
}

fn current_payload_contract_reusing(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&super::payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    cached: &mut Option<CachedHistory>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut failure = "descriptor machine is absent";
    let result = (|| {
        let machine = descriptor?;
        let identity = machine.id;
        failure = "payload query reachability is invalid or exceeds its finite budget";
        let mut relevant = match incoming {
            Some(incoming) => {
                super::payload_reachability::predecessors_with_incoming(machine, incoming, position)?
            }
            None => super::payload_reachability::predecessors(machine, position)?,
        };
        let previous = cached.take();
        let frontier = previous
            .as_ref()
            .and_then(|previous| relevant.compatible_frontier(&previous.relevant, &machine.flow));
        let (mut snapshots, mut pending, previous_relevant) = if let Some(frontier) = frontier {
            let previous = previous.expect("frontier owns the preceding immutable history");
            (previous.snapshots, frontier, Some(previous.relevant))
        } else {
            // Release an incompatible prefix before constructing its replacement.
            drop(previous);
            let mut initial = State::default();
            if let Some((_, requirements)) = &machine.payload_entry_predicates {
                initial.scalar_requirements = Arc::new(
                    super::scalar_contracts::scalar_conjuncts(requirements)
                        .into_iter()
                        .map(Arc::new)
                        .collect(),
                );
            }
            canonicalize_snapshot(&mut initial.scalar_requirements);
            for place in &machine.places {
                if matches!(place.definition, DescriptorDefinition::Parameter { .. }) {
                    failure = "entry payload contract is absent";
                    let mut contract = source(place.place)?;
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
                // No later suffix operation reads this raw producer's
                // premises. Scalar writes/reads inspect observations only;
                // publication below attaches the current ambient theory.
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
                            value: value.clone(),
                        });
                }
            }
            if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                for (location, value) in reads {
                    if let Some(contract) = after.values.get_mut(&location.root) {
                        if let Some(equality) = observe_scalar(contract, &location.path, value) {
                            Arc::make_mut(&mut after.scalar_requirements).push(Arc::new(equality));
                        }
                    }
                }
            }
            if let Some(requirements) = machine.payload_scalar_postconditions.get(&current)
                && !requirements.is_empty()
            {
                Arc::make_mut(&mut after.scalar_requirements)
                    .extend(requirements.iter().cloned().map(Arc::new));
            }
            // Reads can add equations even when this step has no postcondition.
            // Normalize at publication, after all local append operations.
            canonicalize_snapshot(&mut after.scalar_requirements);
            after.publish_requirements();
            for contract in after.values.values_mut() {
                super::payload_predicates::close_with_shared_requirements(
                    types,
                    domains,
                    contract,
                    &after.shared_requirements,
                    None,
                );
            }
            // Only distinct arrivals need independent state. Move the completed
            // transfer into the last relevant edge instead of cloning every
            // payload's full scalar theory even on a straight-line step.
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
                        super::scalar_contracts::scalar_conjuncts(std::slice::from_ref(predicate))
                            .into_iter()
                            .map(Arc::new),
                    );
                    canonicalize_snapshot(&mut arrival.scalar_requirements);
                    for contract in arrival.values.values_mut() {
                        super::payload_predicates::close_with_shared_requirements(
                            types,
                            domains,
                            contract,
                            &arrival.shared_requirements,
                            Some(predicate),
                        );
                    }
                }
                // Snapshot only actual binding results before retiring any
                // formal. Loop edges can read and rebind the same root, so the
                // entire binding batch must observe the pre-arrival state.
                // The other payloads need no second full-state snapshot.
                let mut bindings = Vec::new();
                for (actual, parameter) in &edge.bindings {
                    failure = "edge formal is not a whole payload root";
                    if !parameter.path.is_empty() {
                        return None;
                    }
                    if let Some(contract) =
                        arrival
                            .contract(actual.root, edge_predicate)
                            .and_then(|contract| {
                                select_payload_memberships(types, domains, &contract, &actual.path)
                            })
                    {
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
                // A view selection is a readable set of payload candidates, not
                // an exact address. Intersect its actual source memberships while
                // keeping descriptor custody's exact-only bindings untouched.
                if let Some(additional) = machine
                    .payload_arrivals
                    .get(&current)
                    .and_then(|edges| edges.get(ordinal))
                {
                    for binding in additional {
                        let contract = binding.source.as_ref().and_then(|query| {
                            let targets =
                                analysis.arrival_payload_targets(identity, current, query)?;
                            payload_memberships(types, domains, targets, |root| {
                                arrival.contract(root, edge_predicate)
                            })
                        });
                        if let Some(contract) = contract {
                            failure = "selected edge actual and formal payload carriers differ";
                            let contract = contract.presented(
                                types,
                                domains,
                                *machine.place_types.get(&binding.parameter)?,
                            )?;
                            bindings.push((binding.parameter, contract));
                        }
                    }
                }
                // Readers above see exactly the old ordered union plus its
                // unsorted predicate tail. Retain only that tail in old locals,
                // before replacing formals, so subsequent retirement still
                // removes old instances without copying the shared theory into
                // every unrelated payload. Captured RHS contracts already own
                // their complete pre-rebinding theory.
                if let Some(predicate) = edge_predicate {
                    for contract in arrival.values.values_mut() {
                        if !contract.scalar_requirements.contains(predicate)
                            && !arrival
                                .shared_requirements
                                .iter()
                                .any(|premise| premise.as_ref() == predicate)
                        {
                            contract.scalar_requirements.push(predicate.clone());
                        }
                    }
                }
                for parameter in &edge.parameters {
                    arrival.values.remove(&parameter.root);
                }
                for (parameter, contract) in bindings {
                    // All RHS readers have finished. The rebound formals are
                    // retired and published before another premise read.
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
                        let rebound = super::scalar_contracts::arrival_requirements(
                            &incoming_requirements,
                            bindings,
                            parameters,
                        );
                        // Rebinding can return the complete incoming theory
                        // unchanged. Appending it would detach and deep-clone
                        // a snapshot only to deduplicate it immediately below.
                        // Still compute every substitution: even an empty map
                        // can normalize a proposition into a genuinely new row.
                        if !contains_rebound_premises(&arrival.scalar_requirements, &rebound) {
                            Arc::make_mut(&mut arrival.scalar_requirements)
                                .extend(rebound.into_iter().map(Arc::new));
                        }
                        canonicalize_snapshot(&mut arrival.scalar_requirements);
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
                }
                arrival.publish_requirements();
                for contract in arrival.values.values_mut() {
                    super::payload_predicates::close_with_shared_requirements(
                        types,
                        domains,
                        contract,
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
        *cached = Some(CachedHistory {
            relevant,
            snapshots,
        });
        failure = "requested payload root has no reached incoming membership history";
        let mut current = cached
            .as_ref()?
            .snapshots
            .get(&position)?
            .contract(root, None)?;
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

/// Rebinding usually preserves the incoming roster's order. Check that order
/// before testing its complete theory by a single ordered scan. Normalization
/// can reorder IEEE/math propositions, so unordered results retain the exact
/// binary-search path. Equal duplicates reuse one retained row; no input moves.
fn contains_rebound_premises(
    retained: &[Arc<semantic_vocabulary::Proposition>],
    rebound: &[semantic_vocabulary::Proposition],
) -> bool {
    if rebound.windows(2).any(|pair| pair[0] > pair[1]) {
        return rebound.iter().all(|premise| {
            retained
                .binary_search_by(|stored| stored.as_ref().cmp(premise))
                .is_ok()
        });
    }
    let mut remaining = retained.iter().map(Arc::as_ref).peekable();
    rebound.iter().all(|premise| {
        while remaining
            .peek()
            .is_some_and(|candidate| *candidate < premise)
        {
            remaining.next();
        }
        remaining
            .peek()
            .is_some_and(|candidate| *candidate == premise)
    })
}

/// Union two sorted, unique premise rosters without cloning and then discarding
/// every ambient premise already present in each live payload. Callers normalize
/// newly produced or rebound contracts before this operation; retained snapshots
/// already satisfy that invariant. Preserve the existing proposition on equality,
/// just as stable append/sort/dedup did. No new proposition is established here.
pub(super) fn extend_canonical_premises<Additional: Borrow<semantic_vocabulary::Proposition>>(
    retained: &mut Vec<semantic_vocabulary::Proposition>,
    additional: &[Additional],
) {
    let mut existing = retained.iter().peekable();
    let introduces_premise = additional
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .any(|premise| {
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

/// Intersect canonical premise rosters without searching the whole arrival for
/// every retained premise. The caller publishes sorted, unique snapshots.
fn retain_common_snapshot(
    retained: &mut Arc<Vec<Arc<semantic_vocabulary::Proposition>>>,
    incoming: &[Arc<semantic_vocabulary::Proposition>],
) -> bool {
    let mut remaining = incoming.iter().peekable();
    let unchanged = retained.iter().all(|premise| {
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
    if unchanged {
        return false;
    }
    retain_common_premises(Arc::make_mut(retained), incoming)
}

/// Preserve a published allocation when normalization would change no row.
/// The check uses the same complete proposition ordering as sort/dedup.
fn canonicalize_snapshot(premises: &mut Arc<Vec<Arc<semantic_vocabulary::Proposition>>>) {
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

/// Intersect an owned incoming roster with a snapshot's disjoint local and
/// common rosters, without expanding that common theory into every payload.
/// Equal propositions have the same complete value, including scalar identity.
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
    if let Some(previous) = contract.scalar_observations.iter().find(|observation| {
        observation.path == path
            && observation.value.scalar_type() == value.scalar_type()
            && matches!(
                value.scalar_type(),
                semantic_vocabulary::ScalarType::Integer(_)
                    | semantic_vocabulary::ScalarType::Boolean
            )
    }) {
        return (previous.value != *value).then(|| {
            semantic_vocabulary::Proposition::Equal(value.clone(), previous.value.clone())
        });
    }
    contract
        .scalar_observations
        .retain(|observation| observation.path != path);
    contract
        .scalar_observations
        .push(super::PayloadScalarObservation {
            path: path.to_vec(),
            value: value.clone(),
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
    if state
        .scalar_requirements
        .iter()
        .any(|premise| premise.any_value_id(|value| definitions.contains(&value)))
    {
        Arc::make_mut(&mut state.scalar_requirements)
            .retain(|premise| !premise.any_value_id(|value| definitions.contains(&value)));
    }
    if state
        .shared_requirements
        .iter()
        .any(|premise| !survives_payload_retirement(premise, definitions))
    {
        retire_payload_premises(Arc::make_mut(&mut state.shared_requirements), definitions);
    }
    for contract in state.values.values_mut() {
        contract.scalar_observations.retain(|observation| {
            !observation
                .value
                .any_value_id(|value| definitions.contains(&value))
        });
        retire_payload_premises(&mut contract.scalar_requirements, definitions);
    }
}

fn retire_payload_premises<Premise: Borrow<semantic_vocabulary::Proposition>>(
    premises: &mut Vec<Premise>,
    definitions: &[semantic_vocabulary::ValueId],
) {
    // Payloads also forget unknown/unclosed premises. The ambient filter is
    // deliberately different; applying it here would preserve stale theory.
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
mod tests {
    use super::*;
    use crate::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorPathSegment};
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
    };

    fn position(block: u64) -> DescriptorPosition {
        (BlockId::new(block).unwrap(), 0)
    }

    fn condition(value: u64) -> Proposition {
        Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
        )
    }

    fn forked_history(branches: &[u64]) -> (DescriptorMachine, PayloadMembershipContract) {
        let root = PlaceId::new(1).unwrap();
        let carrier = StructuralTypeId::new(1).unwrap();
        let step = |targets: &[u64]| DescriptorFlowStep {
            successors: targets
                .iter()
                .map(|target| DescriptorFlowEdge {
                    target: position(*target),
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                })
                .collect(),
            ..Default::default()
        };
        let machine = DescriptorMachine {
            id: MachineId::new(1).unwrap(),
            returns: None,
            places: vec![DescriptorPlace {
                place: root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            }],
            place_types: BTreeMap::from([(root, carrier)]),
            entry: position(1),
            flow: BTreeMap::from([
                (position(1), step(branches)),
                (position(2), step(&[4])),
                (position(3), step(&[4])),
                (position(4), step(&[])),
            ]),
            originations: BTreeMap::new(),
            projections: BTreeMap::new(),
            contents_queries: BTreeMap::new(),
            contents_transfers: BTreeMap::new(),
            payload_producers: BTreeMap::new(),
            payload_entry_predicates: Some((
                semantic_vocabulary::PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Boolean),
                    (ValueId::new(2).unwrap(), ScalarType::Boolean),
                ])
                .unwrap(),
                vec![condition(2)],
            )),
            payload_scalar_definitions: BTreeMap::new(),
            payload_scalar_postconditions: BTreeMap::from([(position(2), vec![condition(1)])]),
            payload_scalar_parameters: BTreeMap::new(),
            scalar_payload_reads: BTreeMap::new(),
            payload_edge_predicates: BTreeMap::new(),
            payload_scalar_bindings: BTreeMap::new(),
            scalar_payload_writes: BTreeMap::new(),
            payload_invalidations: BTreeMap::new(),
            payload_arrivals: BTreeMap::new(),
            unresolved_contents_writes: BTreeSet::new(),
            storage_derivations: BTreeSet::new(),
            calls: BTreeMap::new(),
            normal_exits: vec![position(4)],
        };
        let contract = PayloadMembershipContract {
            carrier,
            scalar_requirements: vec![condition(1)],
            scalar_observations: Vec::new(),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        (machine, contract)
    }

    #[test]
    fn entry_observation_does_not_inherit_an_unpublished_ambient_premise() {
        let (machine, source) = forked_history(&[2, 3]);
        let observed = current_payload_contract(
            &[],
            &[],
            std::slice::from_ref(&machine),
            machine.id,
            machine.entry,
            PlaceId::new(1).unwrap(),
            &|_| Some(source.clone()),
        )
        .unwrap();
        assert_eq!(observed.scalar_requirements, vec![condition(1)]);
    }

    #[test]
    fn joined_payload_keeps_both_paths_facts_but_not_a_retired_instance() {
        for branches in [[2, 3], [3, 2]] {
            for retire in [false, true] {
                let (mut machine, source) = forked_history(&branches);
                if retire {
                    machine
                        .payload_scalar_definitions
                        .insert(position(3), vec![ValueId::new(1).unwrap()]);
                }
                let observed = current_payload_contract(
                    &[],
                    &[],
                    std::slice::from_ref(&machine),
                    machine.id,
                    position(4),
                    PlaceId::new(1).unwrap(),
                    &|_| Some(source.clone()),
                )
                .unwrap();
                let mut expected = vec![condition(2)];
                if !retire {
                    expected.push(condition(1));
                }
                expected.sort();
                assert_eq!(observed.scalar_requirements, expected);
            }
        }
    }

    #[test]
    fn predicate_edge_captures_both_arrival_routes_before_instance_retirement() {
        use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

        let root = PlaceId::new(1).unwrap();
        let selected = PlaceId::new(2).unwrap();
        let predicate = Proposition::Conjunction(vec![condition(3), condition(4)]);
        for retire in [false, true] {
            let (mut machine, source) = forked_history(&[2]);
            let types = [StructuralTypeDeclaration {
                quotient_carrier: None,
                id: source.carrier,
                identity: TerminalIdentityIndex(1),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: false,
                descriptor_cleanup: true,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                scalar_restriction: None,
            }];
            machine.places.push(DescriptorPlace {
                place: selected,
                definition: DescriptorDefinition::Local,
            });
            machine.place_types.insert(selected, source.carrier);
            let edge = &mut machine.flow.get_mut(&position(2)).unwrap().successors[0];
            edge.parameters = vec![
                DescriptorLocation::whole(root),
                DescriptorLocation::whole(selected),
            ];
            edge.bindings = vec![(
                DescriptorLocation::whole(root),
                DescriptorLocation::whole(root),
            )];
            machine.payload_arrivals.insert(
                position(2),
                vec![vec![PayloadMembershipBinding {
                    parameter: selected,
                    source: Some(DescriptorContentsQuery {
                        root,
                        steps: Vec::new(),
                    }),
                }]],
            );
            machine
                .payload_edge_predicates
                .insert(position(2), vec![Some(predicate.clone())]);
            if retire {
                machine
                    .payload_scalar_parameters
                    .insert(position(4).0, vec![ValueId::new(3).unwrap()]);
            }
            let mut expected = vec![condition(1), condition(2), condition(4)];
            if !retire {
                expected.extend([condition(3), predicate.clone()]);
            }
            expected.sort();
            for payload in [root, selected] {
                let observed = current_payload_contract(
                    &types,
                    &[],
                    std::slice::from_ref(&machine),
                    machine.id,
                    position(4),
                    payload,
                    &|_| Some(source.clone()),
                )
                .unwrap();
                assert_eq!(observed.scalar_requirements, expected);
            }
        }
    }

    #[test]
    fn retained_payload_snapshot_survives_later_writes_retirement_and_closure() {
        use semantic_vocabulary::{PropositionId, StructuralDomainId};

        let root = PlaceId::new(1).unwrap();
        let path = vec![DescriptorPathSegment::FixedIndex(0)];
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Boolean);
        let domain = |id| StructuralDomainId::new(id).unwrap();
        let mut requirements = vec![
            condition(1),
            Proposition::Atom(PropositionId::new(1).unwrap()),
        ];
        requirements.sort();
        let original = PayloadMembershipContract {
            carrier: StructuralTypeId::new(1).unwrap(),
            scalar_observations: vec![super::super::PayloadScalarObservation {
                path: path.clone(),
                value: value(1),
            }],
            scalar_requirements: requirements,
            qualifications: vec![domain(2), domain(1), domain(2)],
            projected_qualifications: Vec::new(),
        };
        let retained = State {
            values: BTreeMap::from([(root, original.clone())]),
            ..Default::default()
        };
        enum Change {
            Store,
            UnknownWrite,
            RetireInstance,
            Close,
        }
        for change in [
            Change::Store,
            Change::UnknownWrite,
            Change::RetireInstance,
            Change::Close,
        ] {
            let mut later = retained.clone();
            let mut expected = original.clone();
            match change {
                Change::Store => {
                    let contract = later.values.get_mut(&root).unwrap();
                    retire(contract, &[StructuralPathSegment::FixedIndex(0)], &path);
                    assert_eq!(observe_scalar(contract, &path, &value(2)), None);
                    expected.scalar_observations[0].value = value(2);
                    expected.qualifications.clear();
                }
                Change::UnknownWrite => {
                    clear(&mut later.values);
                    expected.scalar_observations.clear();
                    expected.qualifications.clear();
                }
                Change::RetireInstance => {
                    retire_scalar_instances(&mut later, &[ValueId::new(1).unwrap()]);
                    expected.scalar_observations.clear();
                    // Opaque local premises also retire; the ambient filter is
                    // deliberately insufficient for this payload-local theory.
                    expected.scalar_requirements.clear();
                }
                Change::Close => {
                    super::super::payload_predicates::close_with_shared_requirements::<Proposition>(
                        &[],
                        &[],
                        later.values.get_mut(&root).unwrap(),
                        &[],
                        None,
                    );
                    expected.qualifications = vec![domain(1), domain(2)];
                }
            }
            assert_eq!(retained.contract(root, None).unwrap(), original);
            assert_eq!(later.contract(root, None).unwrap(), expected);
        }
    }

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
        assert_eq!(contract.scalar_observations[0].value, value(4));
        let root = PlaceId::new(1).unwrap();
        let mut state = State {
            values: BTreeMap::from([(root, other_branch)]),
            scalar_requirements: Arc::default(),
            shared_requirements: Arc::default(),
        };
        retire_scalar_instances(&mut state, &[ValueId::new(1).unwrap()]);
        assert_eq!(
            observe_scalar(state.values.get_mut(&root).unwrap(), &path, &value(5)),
            None
        );
    }
}
