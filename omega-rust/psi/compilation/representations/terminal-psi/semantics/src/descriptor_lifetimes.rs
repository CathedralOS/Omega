//! Observable descriptor cohorts over an independently reconstructed graph.
//!
//! Consumers provide exact current definitions, operand uses, alias transport,
//! and escapes. This pure replay tracks value instances through simultaneous
//! assignments; it grants no access and supplies no source or proof authority.

use semantic_vocabulary::{BlockId, PlaceId, StructuralCaseId, StructuralFieldId};
use std::collections::{BTreeMap, BTreeSet};

mod locations;
pub use locations::{descriptor_location, descriptor_location_type, descriptor_path};

pub type DescriptorPosition = (BlockId, u32);

/// Canonical carrier address. This is not a backing-storage projection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DescriptorLocation {
    pub root: PlaceId,
    pub path: Vec<DescriptorPathSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DescriptorPathSegment {
    Referent,
    Field(StructuralFieldId),
    FixedIndex(u64),
    Case(StructuralCaseId),
}

impl DescriptorLocation {
    pub fn whole(root: PlaceId) -> Self {
        Self {
            root,
            path: Vec::new(),
        }
    }
    pub fn contains(&self, other: &Self) -> bool {
        self.root == other.root && other.path.starts_with(&self.path)
    }
    pub fn transported(&self, source: &Self, destination: &Self) -> Option<Self> {
        if !source.contains(self) {
            return None;
        }
        let mut path = destination.path.clone();
        path.extend_from_slice(&self.path[source.path.len()..]);
        Some(Self {
            root: destination.root,
            path,
        })
    }
}

#[derive(Clone, Default)]
pub struct DescriptorFlowStep {
    pub definitions: Vec<DescriptorLocation>,
    pub derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
    /// Exact transparent carrier transport. A stored field lends no authority
    /// for its siblings or the containing record.
    pub authority_derivations: Vec<(DescriptorLocation, DescriptorLocation)>,
    pub inputs: Vec<DescriptorLocation>,
    pub may_escape: bool,
    pub successors: Vec<DescriptorFlowEdge>,
}

#[derive(Clone)]
pub struct DescriptorFlowEdge {
    pub target: DescriptorPosition,
    pub parameters: Vec<DescriptorLocation>,
    pub bindings: Vec<(DescriptorLocation, DescriptorLocation)>,
}

impl DescriptorFlowEdge {
    pub fn forward(&self, source: &DescriptorLocation) -> Vec<DescriptorLocation> {
        let mut result = Vec::new();
        if !self
            .parameters
            .iter()
            .any(|parameter| parameter.contains(source))
        {
            result.push(source.clone());
        }
        result.extend(
            self.bindings
                .iter()
                .filter_map(|(actual, parameter)| source.transported(actual, parameter)),
        );
        result
    }
}

pub fn descriptor_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: PlaceId,
) -> BTreeSet<DescriptorPosition> {
    descriptor_location_live_positions(steps, birth, &DescriptorLocation::whole(descriptor))
}

pub fn descriptor_location_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> BTreeSet<DescriptorPosition> {
    // Materialize only reachable (position, name) links. Returning to the
    // formation kills its old result name, not independently carried aliases.
    type Occurrence = (DescriptorPosition, DescriptorLocation);
    let mut pending = Vec::new();
    if let Some(step) = steps.get(&birth) {
        for edge in &step.successors {
            pending.extend(
                edge.forward(descriptor)
                    .into_iter()
                    .map(|name| (edge.target, name)),
            );
        }
    }
    let mut visited = BTreeSet::new();
    let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
    let mut observations = Vec::new();
    let mut escaped = Vec::new();
    while let Some(occurrence) = pending.pop() {
        let (position, name) = &occurrence;
        if !visited.insert(occurrence.clone()) {
            continue;
        }
        let Some(step) = steps.get(position) else {
            continue;
        };
        if step
            .inputs
            .iter()
            .any(|input| input.contains(name) || name.contains(input))
        {
            observations.push(occurrence.clone());
            if step.may_escape {
                escaped.push(*position);
            }
        }
        let mut outputs = Vec::new();
        if !step
            .definitions
            .iter()
            .any(|definition| definition.contains(name))
        {
            outputs.push(name.clone());
        }
        outputs.extend(
            step.derivations
                .iter()
                .filter_map(|(source, result)| name.transported(source, result)),
        );
        for edge in &step.successors {
            for output in &outputs {
                for destination in edge.forward(output) {
                    let next = (edge.target, destination);
                    predecessors
                        .entry(next.clone())
                        .or_default()
                        .insert(occurrence.clone());
                    pending.push(next);
                }
            }
        }
    }
    let mut observed = BTreeSet::new();
    let mut live = BTreeSet::new();
    while let Some(occurrence) = observations.pop() {
        let position = occurrence.0;
        if !observed.insert(occurrence.clone()) {
            continue;
        }
        live.insert(position);
        observations.extend(predecessors.get(&occurrence).into_iter().flatten().cloned());
    }
    // A name kill cannot retire an untracked escape. Its possible alias
    // persists along every subsequent control edge until activation exit.
    let mut escaped_positions = BTreeSet::new();
    while let Some(position) = escaped.pop() {
        if !escaped_positions.insert(position) {
            continue;
        }
        live.insert(position);
        if let Some(step) = steps.get(&position) {
            escaped.extend(step.successors.iter().map(|edge| edge.target));
        }
    }
    live
}

/// Exact descendant names at each input position, established on every incoming
/// path. May-alias/lifetime capture is insufficient for this exemption. A static
/// birth that can execute again cannot identify one resource instance here.
pub fn descriptor_authorized_places(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: PlaceId,
) -> BTreeMap<DescriptorPosition, BTreeSet<PlaceId>> {
    descriptor_location_authorized_locations(steps, birth, &DescriptorLocation::whole(descriptor))
        .into_iter()
        .map(|(position, locations)| {
            (
                position,
                locations
                    .into_iter()
                    .filter(|location| location.path.is_empty())
                    .map(|location| location.root)
                    .collect(),
            )
        })
        .collect()
}

pub fn descriptor_location_authorized_locations(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: &DescriptorLocation,
) -> BTreeMap<DescriptorPosition, BTreeSet<DescriptorLocation>> {
    let Some(formation) = steps.get(&birth) else {
        return BTreeMap::new();
    };
    if !formation
        .definitions
        .iter()
        .any(|definition| definition.contains(descriptor))
    {
        return BTreeMap::new();
    }
    let mut pending = formation
        .successors
        .iter()
        .map(|edge| edge.target)
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(position) = pending.pop() {
        if position == birth {
            return BTreeMap::new();
        }
        if visited.insert(position)
            && let Some(step) = steps.get(&position)
        {
            pending.extend(step.successors.iter().map(|edge| edge.target));
        }
    }
    let mut predecessors =
        BTreeMap::<DescriptorPosition, Vec<(DescriptorPosition, &DescriptorFlowEdge)>>::new();
    for (position, step) in steps {
        for edge in &step.successors {
            predecessors
                .entry(edge.target)
                .or_default()
                .push((*position, edge));
        }
    }
    let mut established = steps
        .keys()
        .map(|position| (*position, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for position in steps.keys().copied() {
            if position == birth {
                continue;
            }
            let mut incoming =
                predecessors
                    .get(&position)
                    .into_iter()
                    .flatten()
                    .map(|(predecessor, edge)| {
                        let step = &steps[predecessor];
                        let before = &established[predecessor];
                        let mut after = transport_authority(
                            before,
                            &step.definitions,
                            &step.authority_derivations,
                        );
                        if *predecessor == birth {
                            after.insert(descriptor.clone());
                        }
                        transport_authority(&after, &edge.parameters, &edge.bindings)
                    });
            let mut current = incoming.next().unwrap_or_default();
            for other in incoming {
                current.retain(|location| other.contains(location));
            }
            if established.get(&position) != Some(&current) {
                established.insert(position, current);
                changed = true;
            }
        }
        if !changed {
            return established;
        }
    }
}

fn transport_authority(
    before: &BTreeSet<DescriptorLocation>,
    definitions: &[DescriptorLocation],
    derivations: &[(DescriptorLocation, DescriptorLocation)],
) -> BTreeSet<DescriptorLocation> {
    let mut after = before
        .iter()
        .filter(|location| {
            !definitions
                .iter()
                .any(|definition| definition.contains(location))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let candidates = before
        .iter()
        .flat_map(|location| {
            derivations
                .iter()
                .filter_map(move |(source, destination)| location.transported(source, destination))
        })
        .collect::<BTreeSet<_>>();
    for candidate in candidates {
        let sources = derivations
            .iter()
            .filter_map(|(source, destination)| candidate.transported(destination, source))
            .collect::<Vec<_>>();
        if !sources.is_empty() && sources.iter().all(|source| before.contains(source)) {
            after.insert(candidate);
        }
    }
    after
}

#[cfg(test)]
mod tests;
