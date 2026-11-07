//! A completed result's original contents are not interchangeable with equal
//! reconstructed data. This query proves only that one produced storage home
//! has not been written since its latest establishment on every incoming path.
//! The consumer separately authenticates the producer, selected call, result
//! type and custody. In particular this does not authorize a profile or memory.

use super::{
    DescriptorAnalysis, DescriptorContentsQuery, DescriptorMachine, DescriptorPayloadTarget,
    descriptor_machine,
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

/// Prepared immutable scalar-version flow for one canonical machine. Queries
/// borrow no second graph, so equal coordinates from another graph cannot reuse
/// this analysis. Topology, value definitions and arrival rosters are prepared
/// once; scratch storage and completed query answers are reused.
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

    /// A real operation completed before the observation on every incoming
    /// path, and none of these scalar operands renewed afterwards. The caller
    /// authenticates that operation and its captured operands separately. This
    /// conservative query retains every syntactic path; it does not assume a
    /// missing or contradictory path in order to prove a requested capture.
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

    // The legacy wrapper retains its query-specific proved Boolean exclusions.
    // It uses this same scalar renewal rule, without reusing an all-path cache
    // answer for a different predecessor domain.
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
                let joined = self.versions[edge.target].map_or(arrival, |old| old && arrival);
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

/// A current borrow resolves to this exact whole produced storage version.
/// The producer's operation kind, selected provider and original input custody
/// are deliberately absent: the consumer must authenticate those separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProducedPayloadVersion {
    pub producer: DescriptorPosition,
    pub payload: DescriptorLocation,
}

/// Resolve an observed payload through the ordinary reference graph and retain
/// its unchanged producer. Equal copies retain their own producer, not the
/// copied value's provenance. Ambiguous aliases and relocated values need a
/// separate occurrence/custody transport proof; this query never guesses one.
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
    let [DescriptorPayloadTarget::Storage(payload)] = targets.as_slice() else {
        return Err("produced payload borrow does not resolve to one original storage home");
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

/// Require the exact current contents of a whole produced value. Descriptor
/// aliases are resolved at each write, rather than compared by operand place.
/// An independent value copy has another home and cannot satisfy this query for
/// the original producer. Genuine value relocation is a separate transport
/// relationship, not inferred from byte equality by this storage-version test.
/// The supplied graph must retain ordinary reference slots.
pub fn require_unchanged_produced_payload(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    payload: &DescriptorLocation,
) -> Result<(), &'static str> {
    let machine =
        descriptor_machine(graph, identity).ok_or("produced payload machine is absent")?;
    let mut analysis = DescriptorAnalysis::with_reference_slots(types, graph);
    require_version(machine, &mut analysis, producer, observation, payload)
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
        &DescriptorPayloadTarget::Storage(payload.clone()),
    )
}

/// Preserve an existing storage occurrence after a completed observation.
/// Unlike a produced-result query, the storage may be an owned formal. The
/// caller must separately establish that the starting operation only borrows
/// this storage and completes normally; this query does not grant that fact.
/// Writes, replacement, unresolved aliases and incoming formal assignments
/// between that completion and the use all retire the observation.
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
    )
}

/// Preserve the incoming contents from before the entry operation. This is an
/// entry-value transport obligation, not a local producer or permission to read
/// a borrowed input. In particular a write by the first operation is checked;
/// it cannot be skipped by pretending entry was a completed establishment.
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
    let machine = descriptor_machine(graph, identity).ok_or("entry payload machine is absent")?;
    require_unchanged_from(machine, analysis, VersionStart::Entry, observation, payload)
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
) -> Result<(), &'static str> {
    let identity = machine.id;
    let relevant = super::payload_reachability::predecessors(machine, observation)
        .ok_or("produced payload observation has unresolved control flow")?;
    // A symbolic incoming referent is not a local allocation or an address.
    // Its exact query identifies the same borrowed payload at both uses. Keep
    // the descriptor binding unchanged too; rebinding it would change which
    // original storage the observation described. Unknown incoming aliases
    // still invalidate the observation rather than proving disjointness.
    let home = match payload {
        DescriptorPayloadTarget::Storage(location) => location.clone(),
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
                    DescriptorPayloadTarget::Storage(location) => overlaps(location),
                    DescriptorPayloadTarget::Incoming(_) => true,
                }) {
                    return Ok(true);
                }
            }
            // Ordinary structural stores need no scalar-write row. A new,
            // distinct result home is not a mutation of this existing value.
            for (_, destination) in &step.derivations {
                if machine.payload_producers.get(&position) == Some(&destination.root) {
                    continue;
                }
                let Some(destinations) = analysis.storage_origins(identity, position, destination)
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
        Err("produced payload is absent, replaced or possibly modified before this observation")
    }
}

/// Require one actual completed observation on every incoming path. Repeated
/// scalar producers and renewed block formals retire old dynamic SSA instances;
/// this never manufactures a value or treats missing flow as impossible.
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
    // Both constructor failures name absent flow positions; keep the legacy
    // caller diagnostic while delegating scalar renewal to the shared owner.
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
            ScalarCaptureFailure::NotCompleted => "evidence capture has not completed before use",
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

/// One bit per reachable position: every arrival must still have the current
/// producer's version. Re-execution of that producer starts a new version;
/// loops do not preserve a stale observation from an earlier dynamic instance.
fn version_flow(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    producer: DescriptorPosition,
    observation: DescriptorPosition,
    includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    modifies: impl FnMut(DescriptorPosition, &DescriptorFlowStep) -> Result<bool, &'static str>,
    replaces_on_arrival: impl Fn(&crate::descriptors::descriptor_lifetimes::DescriptorFlowEdge) -> bool,
) -> Result<bool, &'static str> {
    version_flow_from(
        flow,
        entry,
        VersionStart::Completed(producer),
        observation,
        includes_edge,
        modifies,
        replaces_on_arrival,
    )
}

fn version_flow_from(
    flow: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    entry: DescriptorPosition,
    start: VersionStart,
    observation: DescriptorPosition,
    includes_edge: impl Fn(DescriptorPosition, usize) -> bool,
    mut modifies: impl FnMut(DescriptorPosition, &DescriptorFlowStep) -> Result<bool, &'static str>,
    replaces_on_arrival: impl Fn(&crate::descriptors::descriptor_lifetimes::DescriptorFlowEdge) -> bool,
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
        let after = if matches!(start, VersionStart::Completed(producer) if current == producer) {
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
            // A transition assigns its formals simultaneously, after the
            // source operation. A binding of equal contents is not the old
            // produced instance; only a separate custody-transport proof may
            // connect those instances. Killing one incoming edge must weaken
            // the join, not remove that edge from reachability.
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

/// Recover actual scalar operands stored into the exact version observed by a
/// read. Arithmetic facts and call guarantees cannot create this transport:
/// the adapter separately authenticates every writer against its operation.
pub fn unchanged_stored_scalar_sources(
    types: &[StructuralTypeDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    value: ValueId,
    actual_writes: impl FnMut(DescriptorPosition) -> Vec<(DescriptorLocation, ValueId)>,
) -> Option<Vec<ValueId>> {
    unchanged_stored_sources(types, graph, identity, value, actual_writes)
}

/// The same storage-version transport for an independently authenticated
/// producer identity. Native output arguments, for example, have an exact call
/// occurrence and output slot but no scalar SSA result. They must not acquire
/// a fabricated ValueId. The consumer authenticates each writer and identity;
/// this reader proves only the unchanged original storage at the scalar read.
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

/// Read the unchanged producing sources of an exact storage projection at a
/// current-body cut, including a projection passed as a call argument or
/// returned directly without a scalar-read operation. This query establishes
/// storage version only, not read access, initialization, producer authenticity
/// or a native call's successful completion. The caller supplies its shared
/// reference/view analysis and authenticates every reported writer.
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

/// Follow an authenticated aggregate writer to one observed scalar projection.
/// The storage owner resolves the actual destination and checks its unchanged
/// version. The caller alone interprets the owned suffix in its producer tag;
/// rejecting a suffix supplies no provenance. Numeric equality cannot create a
/// copy, and this query grants no access or initialized-value authority.
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
            let destinations = if machine.payload_producers.get(&producer) == Some(&written.root) {
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
                &DescriptorPayloadTarget::Storage(home.clone()),
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

/// A plain local carrier has its own address even if a later unresolved call
/// prevents whole-machine contents replay. This proves only the address of
/// its typed scalar/aggregate storage, never the contents or borrow access.
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
mod tests {
    use super::*;
    use crate::descriptors::descriptor_lifetimes::{DescriptorFlowBinding, DescriptorFlowEdge};
    use semantic_vocabulary::BlockId;

    fn position(value: u64) -> DescriptorPosition {
        (BlockId::new(value).unwrap(), 0)
    }

    fn payload() -> DescriptorLocation {
        DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(1).unwrap())
    }

    /// Whether an edge transports the observed location in either direction.
    fn joins(edge: &DescriptorFlowEdge, location: &DescriptorLocation) -> bool {
        edge.parameters
            .iter()
            .any(|parameter| parameter.contains(location) || location.contains(parameter))
    }

    fn copy_graph() -> (Vec<StructuralTypeDeclaration>, DescriptorMachine) {
        use super::super::{DescriptorDefinition, DescriptorPlace};
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
        use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};
        let original = payload();
        let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let carrier = StructuralTypeId::new(1).unwrap();
        let types = vec![StructuralTypeDeclaration {
            quotient_carrier: None,
            id: carrier,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: false,
            descriptor_cleanup: true,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            )),
            scalar_restriction: None,
        }];
        let mut flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[5]), (5, &[])]);
        flow.get_mut(&position(2))
            .unwrap()
            .definitions
            .push(original.clone());
        let copying = flow.get_mut(&position(3)).unwrap();
        copying.definitions.push(copy.clone());
        copying.derivations.push((original.clone(), copy.clone()));
        let machine = DescriptorMachine {
            id: MachineId::new(1).unwrap(),
            returns: None,
            places: [original.root, copy.root]
                .into_iter()
                .map(|place| DescriptorPlace {
                    place,
                    definition: DescriptorDefinition::Local,
                })
                .collect(),
            place_types: BTreeMap::from([(original.root, carrier), (copy.root, carrier)]),
            entry: position(1),
            flow,
            case_constructors: BTreeMap::new(),
            case_successors: BTreeMap::new(),
            payload_producers: BTreeMap::from([
                (position(2), original.root),
                (position(3), copy.root),
            ]),
            originations: BTreeMap::new(),
            projections: BTreeMap::new(),
            contents_queries: BTreeMap::new(),
            contents_transfers: BTreeMap::new(),
            payload_entry_predicates: None,
            payload_scalar_definitions: BTreeMap::new(),
            payload_scalar_postconditions: BTreeMap::new(),
            payload_scalar_parameters: BTreeMap::new(),
            scalar_payload_reads: BTreeMap::new(),
            payload_edge_predicates: BTreeMap::new(),
            payload_scalar_bindings: BTreeMap::new(),
            scalar_payload_writes: BTreeMap::new(),
            payload_invalidations: BTreeMap::new(),
            unresolved_contents_writes: Default::default(),
            storage_derivations: Default::default(),
            calls: BTreeMap::new(),
            normal_exits: vec![position(5)],
        };
        (types, machine)
    }

    #[test]
    fn entry_storage_refuses_overlapping_clobbers_without_tainting_other_homes_or_earlier_cuts() {
        let (types, mut machine) = copy_graph();
        for step in machine.flow.values_mut() {
            step.definitions.clear();
            step.derivations.clear();
        }
        let original = payload();
        let other = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let preserved = |machine: &DescriptorMachine| {
            let graph = std::slice::from_ref(machine);
            let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
            require_unchanged_entry_storage(
                &types,
                graph,
                &mut analysis,
                machine.id,
                position(4),
                &DescriptorPayloadTarget::Storage(original.clone()),
            )
        };
        machine.payload_invalidations.insert(
            position(2),
            vec![Some(super::super::DescriptorContentsQuery::from_location(
                &other,
            ))],
        );
        assert!(preserved(&machine).is_ok());
        machine.payload_invalidations.insert(
            position(2),
            vec![Some(super::super::DescriptorContentsQuery::from_location(
                &original,
            ))],
        );
        assert!(preserved(&machine).is_err());
        // An unresolved mutation is conservative on a path to this read.
        machine
            .payload_invalidations
            .insert(position(2), vec![None]);
        assert!(preserved(&machine).is_err());
        // A later mutation cannot retrospectively replace an earlier snapshot.
        machine.payload_invalidations.clear();
        machine
            .payload_invalidations
            .insert(position(5), vec![None]);
        assert!(preserved(&machine).is_ok());
    }

    #[test]
    fn scalar_formal_renews_on_arrival_not_between_operations_in_its_block() {
        let (_, mut machine) = copy_graph();
        let entry = position(1);
        let capture = (semantic_vocabulary::BlockId::new(2).unwrap(), 0);
        let middle = (capture.0, 1);
        let observation = (capture.0, 2);
        let parameter = ValueId::new(8).unwrap();
        let step = |target: Option<DescriptorPosition>| DescriptorFlowStep {
            successors: target
                .into_iter()
                .map(|target| DescriptorFlowEdge {
                    target,
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                })
                .collect(),
            ..Default::default()
        };
        machine.entry = entry;
        machine.flow = BTreeMap::from([
            (entry, step(Some(capture))),
            (capture, step(Some(middle))),
            (middle, step(Some(observation))),
            (observation, step(None)),
        ]);
        machine.payload_scalar_parameters = BTreeMap::from([(capture.0, vec![parameter])]);
        machine.payload_scalar_definitions.clear();
        let mut analysis = ScalarCaptureAnalysis::new(&machine).unwrap();
        analysis
            .require(capture, observation, &[parameter])
            .unwrap();
        assert!(analysis.require(entry, observation, &[parameter]).is_err());
        require_completed_scalar_capture(
            std::slice::from_ref(&machine),
            machine.id,
            capture,
            observation,
            &[parameter],
        )
        .unwrap();
    }

    #[test]
    fn prepared_scalar_capture_refuses_renewed_operands_without_reusing_old_answers() {
        let (_, mut machine) = copy_graph();
        let status = ValueId::new(8).unwrap();
        machine
            .payload_scalar_definitions
            .insert(position(1), vec![status]);
        let mut original = ScalarCaptureAnalysis::new(&machine).unwrap();
        original
            .require(position(2), position(5), &[status])
            .unwrap();
        original
            .require(position(2), position(5), &[status])
            .unwrap();
        assert_eq!(original.answers.len(), 1);

        machine
            .payload_scalar_definitions
            .insert(position(4), vec![status]);
        let mut renewed = ScalarCaptureAnalysis::new(&machine).unwrap();
        assert!(
            renewed
                .require(position(2), position(5), &[status])
                .is_err()
        );
        assert!(
            renewed
                .require(position(2), position(2), &[status])
                .is_err()
        );
        assert!(renewed.require(position(2), position(5), &[]).is_ok());
    }

    #[test]
    fn entry_version_includes_the_first_operation_write() {
        let (_, machine) = copy_graph();
        let observe = |start, first_writes| {
            version_flow_from(
                &machine.flow,
                machine.entry,
                start,
                position(5),
                |_, _| true,
                |position, _| Ok(first_writes && position == machine.entry),
                |_| false,
            )
            .unwrap()
        };
        assert!(observe(VersionStart::Entry, false));
        assert!(!observe(VersionStart::Entry, true));
        // A completed producer names the post-operation version instead.
        assert!(observe(VersionStart::Completed(machine.entry), true));
    }

    #[test]
    fn opaque_output_identity_needs_unchanged_actual_storage() {
        use semantic_vocabulary::{IntegerSign, IntegerType, ScalarTerm, ScalarType};

        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
        struct NativeOutput(DescriptorPosition, u32);

        let (types, mut machine) = copy_graph();
        let identity = machine.id;
        let read = ValueId::new(7).unwrap();
        machine.scalar_payload_reads.insert(
            position(5),
            vec![(
                payload(),
                ScalarTerm::value(
                    read,
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                ),
            )],
        );
        let mut graph = vec![machine];
        let observe = |graph: &[DescriptorMachine]| {
            unchanged_stored_sources(&types, graph, identity, read, |writer_position| {
                if writer_position == position(2) {
                    vec![(payload(), NativeOutput(writer_position, 3))]
                } else {
                    Vec::new()
                }
            })
        };
        assert_eq!(observe(&graph), Some(vec![NativeOutput(position(2), 3)]));
        // A later opaque write cannot be replaced with an equal asserted
        // output identity. It destroys the original storage-version proof.
        graph[0].unresolved_contents_writes.insert(position(4));
        assert!(observe(&graph).is_none());
    }

    #[test]
    fn copied_projection_names_its_pre_copy_source_and_keeps_its_own_version() {
        use crate::descriptors::descriptor_lifetimes::DescriptorPathSegment;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{ArrayExtent, StructuralTypeShape};
        let (mut types, mut machine) = copy_graph();
        let mut element = types[0].clone();
        element.id = StructuralTypeId::new(2).unwrap();
        types[0].shape = StructuralTypeShape::FixedArray {
            element: element.id,
            length: ArrayExtent::Literal(2),
        };
        types.push(element);
        let original = payload();
        let copy = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let mut observed = copy.clone();
        observed.path.push(DescriptorPathSegment::FixedIndex(1));
        let mut source_leaf = original.clone();
        source_leaf.path = observed.path.clone();
        machine.payload_invalidations.insert(
            position(4),
            vec![Some(DescriptorContentsQuery::from_location(&source_leaf))],
        );
        let observe = |machine: &DescriptorMachine| {
            let graph = std::slice::from_ref(machine);
            let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, graph);
            projected_stored_sources_in(
                &types,
                graph,
                &mut analysis,
                machine.id,
                position(5),
                &observed,
                |cut| {
                    if cut == position(3) {
                        vec![(copy.clone(), (cut, original.clone()))]
                    } else {
                        Vec::new()
                    }
                },
                |(capture, mut source), suffix| {
                    source.path.extend_from_slice(suffix);
                    Some((capture, source))
                },
            )
        };
        // A write to the original after copying cannot redirect the captured
        // source to its new contents. The source cut remains before the copy.
        assert_eq!(observe(&machine), Some(vec![(position(3), source_leaf)]));
        machine.payload_invalidations.insert(
            position(4),
            vec![Some(DescriptorContentsQuery::from_location(&observed))],
        );
        assert!(observe(&machine).is_none());
    }

    #[test]
    fn equal_copies_keep_distinct_producers_and_writes_retire_only_their_contents() {
        let (types, machine) = copy_graph();
        let identity = machine.id;
        let mut graph = vec![machine];
        let original = DescriptorContentsQuery::from_location(&payload());
        let copy = DescriptorContentsQuery::from_location(&DescriptorLocation::whole(
            semantic_vocabulary::PlaceId::new(2).unwrap(),
        ));
        let observe = |graph: &[DescriptorMachine], query| {
            unchanged_payload_producer(&types, graph, identity, position(5), query)
        };
        assert_eq!(observe(&graph, &original).unwrap().producer, position(2));
        assert_eq!(observe(&graph, &copy).unwrap().producer, position(3));
        assert!(
            require_unchanged_produced_payload(
                &types,
                &graph,
                identity,
                position(2),
                position(5),
                &copy.exact_location().unwrap(),
            )
            .is_err()
        );
        graph[0]
            .payload_invalidations
            .insert(position(4), vec![Some(copy.clone())]);
        assert!(observe(&graph, &original).is_ok());
        assert!(observe(&graph, &copy).is_err());
        graph[0]
            .payload_invalidations
            .insert(position(4), vec![None]);
        assert!(observe(&graph, &original).is_err());
        assert!(observe(&graph, &copy).is_err());
    }

    #[test]
    fn whole_borrowed_arrival_keeps_the_original_producer_and_invalidations() {
        use super::super::DescriptorDefinition;
        use crate::descriptors::descriptor_lifetimes::DescriptorFlowSource;
        use terminal_psi::StructuralAccess;

        let (types, mut machine) = copy_graph();
        let identity = machine.id;
        let borrowed = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        machine.originations.insert(
            position(2),
            vec![(
                payload(),
                super::super::DescriptorCallOrigin::Argument {
                    source: payload().root,
                    projection: super::super::Projection::new(&[]),
                },
            )],
        );
        machine.places[1].definition = DescriptorDefinition::Arrival {
            access: StructuralAccess::SharedBorrow,
        };
        machine.payload_producers.remove(&position(3));
        let capture = machine.flow.get_mut(&position(3)).unwrap();
        capture.definitions.clear();
        capture.derivations.clear();
        capture.successors[0].parameters.push(borrowed.clone());
        capture.successors[0].bindings.push(
            DescriptorFlowBinding::new(
                DescriptorFlowSource::Location(payload()),
                borrowed.clone(),
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow,
            )
            .unwrap(),
        );
        let query = DescriptorContentsQuery::from_location(&borrowed);
        let mut graph = vec![machine];
        let observe = |graph: &[DescriptorMachine]| {
            unchanged_payload_producer(&types, graph, identity, position(5), &query)
        };
        assert_eq!(
            observe(&graph).unwrap(),
            ProducedPayloadVersion {
                producer: position(2),
                payload: payload()
            },
        );
        // A write through this alias retires the same original observation.
        graph[0]
            .payload_invalidations
            .insert(position(4), vec![Some(query.clone())]);
        assert!(observe(&graph).is_err());
        graph[0].payload_invalidations.clear();
        // Owned arrival is value transport, not an alias to the old home.
        graph[0].places[1].definition = DescriptorDefinition::Arrival {
            access: StructuralAccess::Owned,
        };
        graph[0].flow.get_mut(&position(3)).unwrap().successors[0].bindings[0] =
            DescriptorFlowBinding::new(
                DescriptorFlowSource::Location(payload()),
                borrowed,
                StructuralAccess::Owned,
                StructuralAccess::Owned,
            )
            .unwrap();
        assert!(observe(&graph).is_err());
    }

    #[test]
    fn ambiguous_producers_and_unfinished_results_do_not_supply_a_version() {
        let (types, machine) = copy_graph();
        let identity = machine.id;
        let mut graph = vec![machine];
        let original = DescriptorContentsQuery::from_location(&payload());
        assert!(
            unchanged_payload_producer(&types, &graph, identity, position(2), &original).is_err()
        );
        graph[0]
            .payload_producers
            .insert(position(4), payload().root);
        assert_eq!(
            unchanged_payload_producer(&types, &graph, identity, position(5), &original),
            Err("observed payload has multiple whole-value producers"),
        );
    }

    #[test]
    fn completed_shared_observation_preserves_existing_storage_not_replacements() {
        let (types, mut machine) = copy_graph();
        let identity = machine.id;
        // The extent was supplied by the caller, not produced by the profile
        // call. Its separate shared-only completion contract is a precondition
        // of this query; no result producer is invented for the extent.
        machine.payload_producers.remove(&position(2));
        machine
            .flow
            .get_mut(&position(2))
            .unwrap()
            .definitions
            .clear();
        let mut graph = vec![machine];
        let observe = |graph: &[DescriptorMachine]| {
            require_unchanged_observed_storage(
                &types,
                graph,
                identity,
                position(2),
                position(5),
                &DescriptorPayloadTarget::Storage(payload()),
            )
        };
        assert!(observe(&graph).is_ok());
        assert!(
            require_unchanged_produced_payload(
                &types,
                &graph,
                identity,
                position(2),
                position(5),
                &payload()
            )
            .is_err()
        );
        assert!(
            require_unchanged_observed_storage(
                &types,
                &graph,
                identity,
                position(2),
                position(2),
                &DescriptorPayloadTarget::Storage(payload())
            )
            .is_err()
        );
        graph[0].payload_invalidations.insert(
            position(4),
            vec![Some(DescriptorContentsQuery::from_location(&payload()))],
        );
        assert!(observe(&graph).is_err());
        graph[0].payload_invalidations.clear();
        graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
            .parameters
            .push(payload());
        assert!(
            observe(&graph).is_err(),
            "an equal replacement is not this observation"
        );
        graph[0].flow.get_mut(&position(4)).unwrap().successors[0]
            .parameters
            .clear();
        graph[0]
            .flow
            .get_mut(&position(1))
            .unwrap()
            .successors
            .push(DescriptorFlowEdge {
                target: position(5),
                parameters: Vec::new(),
                bindings: Vec::new(),
            });
        assert!(
            observe(&graph).is_err(),
            "every arrival must have completed the observation"
        );
    }

    fn flow(edges: &[(u64, &[u64])]) -> BTreeMap<DescriptorPosition, DescriptorFlowStep> {
        edges
            .iter()
            .map(|(source, targets)| {
                (
                    position(*source),
                    DescriptorFlowStep {
                        successors: targets
                            .iter()
                            .map(|target| DescriptorFlowEdge {
                                target: position(*target),
                                parameters: Vec::new(),
                                bindings: Vec::new(),
                            })
                            .collect(),
                        ..Default::default()
                    },
                )
            })
            .collect()
    }

    #[test]
    fn all_arrivals_need_the_completed_unchanged_version() {
        let diamond = flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
        let query = |writes: &[u64]| {
            version_flow(
                &diamond,
                position(1),
                position(2),
                position(5),
                |_, _| true,
                |current, _| Ok(writes.iter().any(|write| position(*write) == current)),
                |edge| joins(edge, &payload()),
            )
            .unwrap()
        };
        assert!(query(&[]));
        assert!(!query(&[3]));
        assert!(!query(&[4]));
        let bypass = flow(&[(1, &[2, 4]), (2, &[3]), (3, &[5]), (4, &[5]), (5, &[])]);
        assert!(
            !version_flow(
                &bypass,
                position(1),
                position(2),
                position(5),
                |_, _| true,
                |_, _| Ok(false),
                |edge| joins(edge, &payload()),
            )
            .unwrap()
        );
    }

    #[test]
    fn transition_replacement_retires_only_the_overlapping_version() {
        let original = payload();
        let other = DescriptorLocation::whole(semantic_vocabulary::PlaceId::new(2).unwrap());
        let field = DescriptorLocation {
            root: original.root,
            path: vec![
                crate::descriptors::descriptor_lifetimes::DescriptorPathSegment::Field(
                    semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                ),
            ],
        };
        for (parameter, retained) in [
            (other.clone(), true),
            (original.clone(), false),
            (field, false),
        ] {
            let mut diamond = flow(&[(1, &[2]), (2, &[3, 4]), (3, &[5]), (4, &[5]), (5, &[])]);
            let edge = &mut diamond.get_mut(&position(4)).unwrap().successors[0];
            edge.parameters.push(parameter.clone());
            edge.bindings.push(
                DescriptorFlowBinding::new(
                    other.clone(),
                    parameter,
                    terminal_psi::StructuralAccess::Owned,
                    terminal_psi::StructuralAccess::Owned,
                )
                .unwrap(),
            );
            assert_eq!(
                version_flow(
                    &diamond,
                    position(1),
                    position(2),
                    position(5),
                    |_, _| true,
                    |_, _| Ok(false),
                    |edge| joins(edge, &original),
                )
                .unwrap(),
                retained,
            );
        }
    }

    #[test]
    fn new_loop_completion_replaces_the_previous_dynamic_version() {
        let loop_flow = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[2, 5]), (5, &[])]);
        assert!(
            version_flow(
                &loop_flow,
                position(1),
                position(2),
                position(3),
                |_, _| true,
                |current, _| Ok(current == position(4)),
                |edge| joins(edge, &payload()),
            )
            .unwrap()
        );
        let stale_loop = flow(&[(1, &[2]), (2, &[3]), (3, &[4]), (4, &[3, 5]), (5, &[])]);
        assert!(
            !version_flow(
                &stale_loop,
                position(1),
                position(2),
                position(3),
                |_, _| true,
                |current, _| Ok(current == position(4)),
                |edge| joins(edge, &payload()),
            )
            .unwrap()
        );
    }
}
