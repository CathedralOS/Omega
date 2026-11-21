use super::{
    DescriptorAnalysis, DescriptorContentsQuery, DescriptorMachine,
    DescriptorPayloadTarget, descriptor_machine,
};
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowStep, DescriptorLocation, DescriptorPosition,
};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::ValueId;
use std::collections::{BTreeMap, VecDeque};
use terminal_psi::StructuralTypeDeclaration;

struct ScalarCaptureEdge {
    target: usize,
    parameters: Vec<ValueId>,
}

struct ScalarCaptureStep {
    definitions: Vec<ValueId>,
    successors: Vec<ScalarCaptureEdge>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarCaptureFailure {
    NotCompleted,
    UnknownPosition(DescriptorPosition),
    MissingOrRenewed,
    QueryBudget,
    InvalidFlow,
}

pub struct ScalarCaptureAnalysis {
    identity: MachineId,
    positions: BTreeMap<DescriptorPosition, usize>,
    coordinates: Vec<DescriptorPosition>,
    entry: usize,
    steps: Vec<ScalarCaptureStep>,
    answers: BTreeMap<(usize, usize, Vec<ValueId>), bool>,
    versions: Vec<Option<bool>>,
    queued: Vec<bool>,
    pending: VecDeque<usize>,
}

impl ScalarCaptureAnalysis {
    pub fn new(machine: &DescriptorMachine) -> Result<Self, &'static str> {
        let positions = machine
            .flow
            .keys()
            .copied()
            .enumerate()
            .map(|(index, position)| (position, index))
            .collect::<BTreeMap<_, _>>();
        let entry = *positions
            .get(&machine.entry)
            .ok_or("scalar capture graph has no entry")?;
        let mut steps = Vec::with_capacity(positions.len());
        for (&position, step) in &machine.flow {
            let mut successors = Vec::with_capacity(step.successors.len());
            for edge in &step.successors {
                let target = *positions
                    .get(&edge.target)
                    .ok_or("scalar capture graph has an absent target")?;
                let parameters = if edge.target.1 == 0 {
                    machine
                        .payload_scalar_parameters
                        .get(&edge.target.0)
                        .cloned()
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                successors.push(ScalarCaptureEdge { target, parameters });
            }
            steps.push(ScalarCaptureStep {
                definitions: machine
                    .payload_scalar_definitions
                    .get(&position)
                    .cloned()
                    .unwrap_or_default(),
                successors,
            });
        }
        Ok(Self {
            identity: machine.id,
            entry,
            versions: vec![None; steps.len()],
            queued: vec![false; steps.len()],
            pending: VecDeque::new(),
            coordinates: positions.keys().copied().collect(),
            positions,
            steps,
            answers: BTreeMap::new(),
        })
    }

    pub fn identity(&self) -> MachineId {
        self.identity
    }

    pub fn require(
        &mut self,
        source: DescriptorPosition,
        observation: DescriptorPosition,
        values: &[ValueId],
    ) -> Result<(), ScalarCaptureFailure> {
        if source == observation {
            return Err(ScalarCaptureFailure::NotCompleted);
        }
        let source = *self
            .positions
            .get(&source)
            .ok_or(ScalarCaptureFailure::UnknownPosition(source))?;
        let observation = *self
            .positions
            .get(&observation)
            .ok_or(ScalarCaptureFailure::UnknownPosition(observation))?;
        let mut values = values.to_vec();
        values.sort();
        values.dedup();
        let key = (source, observation, values);
        if let Some(valid) = self.answers.get(&key) {
            return if *valid {
                Ok(())
            } else {
                Err(ScalarCaptureFailure::MissingOrRenewed)
            };
        }
        if self.answers.len() >= 65_536 {
            return Err(ScalarCaptureFailure::QueryBudget);
        }
        let valid = self.solve(source, observation, &key.2, |_, _| true)?;
        self.answers.insert(key, valid);
        if valid {
            Ok(())
        } else {
            Err(ScalarCaptureFailure::MissingOrRenewed)
        }
    }

    fn require_on_edges(
        &mut self,
        source: DescriptorPosition,
        observation: DescriptorPosition,
        values: &[ValueId],
        includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    ) -> Result<bool, ScalarCaptureFailure> {
        let source = *self
            .positions
            .get(&source)
            .ok_or(ScalarCaptureFailure::UnknownPosition(source))?;
        let observation = *self
            .positions
            .get(&observation)
            .ok_or(ScalarCaptureFailure::UnknownPosition(observation))?;
        let mut values = values.to_vec();
        values.sort();
        values.dedup();
        self.solve(source, observation, &values, includes_edge)
    }

    fn solve(
        &mut self,
        source: usize,
        observation: usize,
        values: &[ValueId],
        includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    ) -> Result<bool, ScalarCaptureFailure> {
        self.versions.fill(None);
        self.queued.fill(false);
        self.pending.clear();
        self.versions[self.entry] = Some(false);
        self.queued[self.entry] = true;
        self.pending.push_back(self.entry);
        while let Some(current) = self.pending.pop_front() {
            self.queued[current] = false;
            let before = self.versions[current].ok_or(ScalarCaptureFailure::InvalidFlow)?;
            let step = &self.steps[current];
            let after = if current == source {
                true
            } else {
                before
                    && !step
                        .definitions
                        .iter()
                        .any(|value| values.binary_search(value).is_ok())
            };
            for (ordinal, edge) in step.successors.iter().enumerate() {
                if !includes_edge(self.coordinates[current], ordinal) {
                    continue;
                }
                let arrival = after
                    && !edge
                        .parameters
                        .iter()
                        .any(|value| values.binary_search(value).is_ok());
                let joined =
                    self.versions[edge.target].map_or(arrival, |old| old && arrival);
                if self.versions[edge.target] != Some(joined) {
                    self.versions[edge.target] = Some(joined);
                    if !self.queued[edge.target] {
                        self.queued[edge.target] = true;
                        self.pending.push_back(edge.target);
                    }
                }
            }
        }
        Ok(self.versions[observation] == Some(true))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducedPayloadVersion {
    pub producer: DescriptorPosition,
    pub payload: DescriptorLocation,
}

pub fn unchanged_payload_producer(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    observation: DescriptorPosition,
    query: &DescriptorContentsQuery,
) -> Result<ProducedPayloadVersion, &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    let targets = analysis
        .payload_targets(identity, observation, query)
        .ok_or("produced payload borrow has unresolved backing")?;
    let [
        DescriptorPayloadTarget::Storage {
            location: payload,
            completed: None,
        },
    ] = targets.as_slice()
    else {
        return Err(
            "produced payload borrow does not resolve to one original storage home",
        );
    };
    let mut producers = machine
        .payload_producers
        .iter()
        .filter(|(_, root)| **root == payload.root);
    let (&producer, _) = producers
        .next()
        .ok_or("observed payload has no local whole-value producer")?;
    if producers.next().is_some() {
        return Err("observed payload has multiple whole-value producers");
    }
    require_version(machine, &mut analysis, producer, observation, payload)?;
    Ok(ProducedPayloadVersion {
        producer,
        payload: payload.clone(),
    })
}

pub fn require_unchanged_produced_payload(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    require_unchanged_produced_payload_in(
        types,
        graph,
        &mut analysis,
        identity,
        producer,
        observation,
        payload,
    )
}

/// Reuse the caller's prepared storage history. A missing or changed
/// version is an error, including an unresolved write on any arrival.
pub fn require_unchanged_produced_payload_in(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    identity: MachineId,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    if !analysis.uses_source_graph(types, graph) {
        return Err("produced storage analysis belongs to another immutable source graph");
    }
    let machine =
        descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
    require_version(machine, analysis, producer, observation, payload)
}

fn require_version(
    machine: &DescriptorMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    if !payload.path.is_empty()
        || machine.payload_producers.get(&producer) != Some(&payload.root)
        || producer == observation
    {
        return Err("payload query does not name a completed whole-value producer");
    }
    require_unchanged_between(
        machine,
        analysis,
        producer,
        observation,
        &DescriptorPayloadTarget::current(payload.clone()),
    )
}

pub fn require_unchanged_observed_storage(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    completed_observation: DescriptorPosition,
    use_position: DescriptorPosition,
    storage: &DescriptorPayloadTarget,
) -> Result<(), &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("observed storage machine is absent")?;
    if completed_observation == use_position {
        return Err("storage observation has not completed before its use");
    }
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    require_unchanged_between(
        machine,
        &mut analysis,
        completed_observation,
        use_position,
        storage,
    )
}

fn require_unchanged_between(
    machine: &DescriptorMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorPayloadTarget,
) -> Result<(), &'static str> {
    require_unchanged_from(
        machine,
        analysis,
        VersionStart::Completed(producer),
        observation,
        payload,
        None,
    )
}

pub fn require_unchanged_entry_storage(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    identity: MachineId,
    observation: DescriptorPosition,
    payload: &DescriptorPayloadTarget,
) -> Result<(), &'static str> {
    if !analysis.uses_source_graph(types, graph) {
        return Err("entry storage analysis belongs to another immutable source graph");
    }
    let machine =
        descriptor_machine(graph, identity).ok_or("entry payload machine is absent")?;
    require_unchanged_from(
        machine,
        analysis,
        VersionStart::Entry,
        observation,
        payload,
        None,
    )
}

pub(crate) fn require_unchanged_entry_storage_with_incoming(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    identity: MachineId,
    observation: DescriptorPosition,
    payload: &DescriptorPayloadTarget,
    incoming: &super::payload_reachability::IncomingFlow,
) -> Result<(), &'static str> {
    if !analysis.uses_source_graph(types, graph) {
        return Err("entry storage analysis belongs to another immutable source graph");
    }
    let machine =
        descriptor_machine(graph, identity).ok_or("entry payload machine is absent")?;
    require_unchanged_from(
        machine,
        analysis,
        VersionStart::Entry,
        observation,
        payload,
        Some(incoming),
    )
}

#[derive(Clone, Copy)]
enum VersionStart {
    Entry,
    Completed(DescriptorPosition),
}

fn require_unchanged_from(
    machine: &DescriptorMachine,
    analysis: &mut DescriptorAnalysis<'_>,
    start: VersionStart,
    observation: DescriptorPosition,
    payload: &DescriptorPayloadTarget,
    incoming: Option<&super::payload_reachability::IncomingFlow>,
) -> Result<(), &'static str> {
    let identity = machine.id;
    let relevant = match incoming {
        Some(incoming) => super::payload_reachability::predecessors_with_incoming(
            machine,
            incoming,
            observation,
        ),
        None => super::payload_reachability::predecessors(machine, observation),
    }
    .ok_or("produced payload observation has unresolved control flow")?;
    let home = match payload {
        DescriptorPayloadTarget::Storage {
            location,
            completed: None,
        } => location.clone(),
        DescriptorPayloadTarget::Storage {
            completed: Some(_), ..
        } => return Err("a captured constructor value is not current mutable storage"),
        DescriptorPayloadTarget::Incoming(query) => DescriptorLocation::whole(query.root),
    };
    let overlaps =
        |location: &DescriptorLocation| home.contains(location) || location.contains(&home);
    let unchanged = version_flow_from(
        &machine.flow,
        machine.entry,
        start,
        observation,
        |position, ordinal| relevant.includes_edge(position, ordinal),
        |position, step| {
            if machine.unresolved_contents_writes.contains(&position) {
                return Ok(true);
            }
            if step.definitions.iter().any(overlaps) {
                return Ok(true);
            }
            for query in machine
                .payload_invalidations
                .get(&position)
                .into_iter()
                .flatten()
            {
                let Some(targets) = query
                    .as_ref()
                    .and_then(|query| analysis.payload_targets(identity, position, query))
                else {
                    return Ok(true);
                };
                if targets.iter().any(|target| match target {
                    DescriptorPayloadTarget::Storage {
                        location,
                        completed: None,
                    } => overlaps(location),
                    DescriptorPayloadTarget::Storage {
                        completed: Some(_), ..
                    } => true,
                    DescriptorPayloadTarget::Incoming(_) => true,
                }) {
                    return Ok(true);
                }
            }
            for (_, destination) in &step.derivations {
                if machine.payload_producers.get(&position) == Some(&destination.root) {
                    continue;
                }
                let Some(destinations) =
                    analysis.storage_origins(identity, position, destination)
                else {
                    return Ok(true);
                };
                if destinations.iter().any(overlaps) {
                    return Ok(true);
                }
            }
            Ok(false)
        },
        |edge| {
            edge.parameters
                .iter()
                .any(|parameter| parameter.contains(&home) || home.contains(parameter))
        },
    )?;
    if unchanged {
        Ok(())
    } else {
        Err(
            "produced payload is absent, replaced or possibly modified before this observation",
        )
    }
}

pub fn require_completed_scalar_capture(
    graph: &[DescriptorMachine],
    identity: MachineId,
    source: DescriptorPosition,
    observation: DescriptorPosition,
    values: &[ValueId],
) -> Result<(), &'static str> {
    if source == observation {
        return Err("evidence capture has not completed before use");
    }
    let machine =
        descriptor_machine(graph, identity).ok_or("evidence capture machine is absent")?;
    let relevant = super::payload_reachability::predecessors(machine, observation)
        .ok_or("evidence capture has unresolved incoming control flow")?;
    let mut analysis = ScalarCaptureAnalysis::new(machine)
        .map_err(|_| "payload version flow names an absent position")?;
    let valid = analysis
        .require_on_edges(source, observation, values, |position, ordinal| {
            relevant.includes_edge(position, ordinal)
        })
        .map_err(|reason| match reason {
            ScalarCaptureFailure::UnknownPosition(_) => {
                "payload version flow names an absent position"
            }
            ScalarCaptureFailure::InvalidFlow => "payload version arrival is absent",
            ScalarCaptureFailure::NotCompleted => {
                "evidence capture has not completed before use"
            }
            ScalarCaptureFailure::MissingOrRenewed => {
                "evidence capture is absent or its SSA instance was renewed before use"
            }
            ScalarCaptureFailure::QueryBudget => "scalar capture query budget exceeded",
        })?;
    if valid {
        Ok(())
    } else {
        Err("evidence capture is absent or its SSA instance was renewed before use")
    }
}

fn version_flow_from(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    start: VersionStart,
    observation: DescriptorPosition,
    includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    mut modifies: impl FnMut(
        DescriptorPosition,
        &DescriptorFlowStep,
    ) -> Result<bool, &'static str>,
    replaces_on_arrival: impl Fn(
        &crate::descriptors::descriptor_lifetimes::DescriptorFlowEdge,
    ) -> bool,
) -> Result<bool, &'static str> {
    let positions = flow.keys().copied().collect::<Vec<_>>();
    let position_index = |position| {
        positions
            .binary_search(&position)
            .map_err(|_| "payload version flow names an absent position")
    };
    let entry_index = position_index(entry)?;
    let observation_index = position_index(observation)?;
    if let VersionStart::Completed(producer) = start {
        position_index(producer)?;
    }
    let mut versions = vec![None; positions.len()];
    let mut writes = vec![None; positions.len()];
    versions[entry_index] = Some(matches!(start, VersionStart::Entry));
    let mut pending = VecDeque::from([entry_index]);
    while let Some(current_index) = pending.pop_front() {
        let current = positions[current_index];
        let step = flow
            .get(&current)
            .ok_or("payload version flow lost its step")?;
        let before = versions[current_index].ok_or("payload version arrival is absent")?;
        let after = if matches!(start, VersionStart::Completed(producer) if current == producer)
        {
            true
        } else if before {
            let modifies = match writes[current_index] {
                Some(modifies) => modifies,
                None => {
                    let result = modifies(current, step)?;
                    writes[current_index] = Some(result);
                    result
                }
            };
            !modifies
        } else {
            false
        };
        for (ordinal, edge) in step.successors.iter().enumerate() {
            if !includes_edge(current, ordinal) {
                continue;
            }
            let target_index = position_index(edge.target)?;
            let arrival = after && !replaces_on_arrival(edge);
            let joined = versions[target_index].map_or(arrival, |old| old && arrival);
            if versions[target_index] != Some(joined) {
                versions[target_index] = Some(joined);
                if !pending.contains(&target_index) {
                    pending.push_back(target_index);
                }
            }
        }
    }
    Ok(versions[observation_index] == Some(true))
}

pub fn unchanged_stored_scalar_sources(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    value: ValueId,
    actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, ValueId)>,
) -> Option<Vec<ValueId>> {
    unchanged_stored_sources(types, graph, identity, value, actual_writes)
}

pub fn unchanged_stored_sources<Source: Ord>(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    value: ValueId,
    actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
) -> Option<Vec<Source>> {
    let machine = descriptor_machine(graph, identity)?;
    let mut reads = machine
        .scalar_payload_reads
        .iter()
        .flat_map(|(position, rows)| {
            rows.iter().filter_map(move |(location, term)| {
                matches!(term, semantic_vocabulary::ScalarTerm::Value { id, .. } if *id == value)
                    .then_some((*position, location))
            })
        });
    let (observation, observed) = reads.next()?;
    if reads.next().is_some() {
        return None;
    }
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    unchanged_stored_projection_sources_in(
        types,
        graph,
        &mut analysis,
        identity,
        observation,
        observed,
        actual_writes,
    )
}

pub fn unchanged_stored_projection_sources_in<Source: Ord>(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    identity: MachineId,
    observation: DescriptorPosition,
    observed: &DescriptorLocation,
    actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
) -> Option<Vec<Source>> {
    projected_stored_sources_in(
        types,
        graph,
        analysis,
        identity,
        observation,
        observed,
        actual_writes,
        |source, suffix| suffix.is_empty().then_some(source),
    )
}

pub fn projected_stored_sources_in<Source: Ord>(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    analysis: &mut DescriptorAnalysis<'_>,
    identity: MachineId,
    observation: DescriptorPosition,
    observed: &DescriptorLocation,
    mut actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, Source)>,
    mut project: impl FnMut(
        Source,
        &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    ) -> Option<Source>,
) -> Option<Vec<Source>> {
    if !analysis.uses_source_graph(types, graph) {
        return None;
    }
    let machine = descriptor_machine(graph, identity)?;
    if !machine.flow.contains_key(&observation) {
        return None;
    }
    let home = direct_owned_storage(types, machine, observed)
        .or_else(|| analysis.location(identity, observation, observed))?;
    let mut sources = Vec::new();
    for &producer in machine.flow.keys() {
        if producer == observation {
            continue;
        }
        for (written, id) in actual_writes(producer) {
            let destinations =
                if machine.payload_producers.get(&producer) == Some(&written.root) {
                    vec![written.clone()]
                } else {
                    let Some(destination) = direct_owned_storage(types, machine, &written)
                        .or_else(|| analysis.location(identity, producer, &written))
                    else {
                        continue;
                    };
                    vec![destination]
                };
            let [destination] = destinations.as_slice() else {
                continue;
            };
            if destination.root != home.root || !home.path.starts_with(&destination.path) {
                continue;
            }
            let Some(id) = project(id, &home.path[destination.path.len()..]) else {
                continue;
            };
            if require_unchanged_between(
                machine,
                analysis,
                producer,
                observation,
                &DescriptorPayloadTarget::current(home.clone()),
            )
            .is_ok()
            {
                sources.push(id);
            }
        }
    }
    sources.sort();
    sources.dedup();
    (!sources.is_empty()).then_some(sources)
}

fn direct_owned_storage(
    types: &[StructuralTypeDeclaration],
    machine: &DescriptorMachine,
    location: &DescriptorLocation,
) -> Option<DescriptorLocation> {
    let place = exactly_one(machine.places.iter(), |row| row.place == location.root)?;
    if !matches!(place.definition, super::DescriptorDefinition::Local) {
        return None;
    }
    let ty = *machine.place_types.get(&location.root)?;
    if !super::plain_owned_shape(types.iter(), ty) {
        return None;
    }
    crate::descriptors::descriptor_lifetimes::descriptor_path(types, ty, &location.path)?;
    Some(location.clone())
}

#[cfg(test)]
mod tests;
