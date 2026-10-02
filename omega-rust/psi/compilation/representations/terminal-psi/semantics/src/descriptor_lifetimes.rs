//! Observable descriptor cohorts over an independently reconstructed graph.
//!
//! Consumers provide exact current definitions, operand uses, alias transport,
//! and escapes. This pure replay tracks value instances through simultaneous
//! assignments; it grants no access and supplies no source or proof authority.

use semantic_vocabulary::{BlockId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};

pub type DescriptorPosition = (BlockId, u32);

#[derive(Default)]
pub struct DescriptorFlowStep {
    pub definitions: Vec<PlaceId>,
    pub derivations: Vec<(PlaceId, PlaceId)>,
    /// Transparent descendant aliases only. Aggregate capture keeps a cohort
    /// live, but cannot authorize accesses to the rest of that aggregate.
    pub authority_derivations: Vec<(PlaceId, PlaceId)>,
    pub inputs: Vec<PlaceId>,
    pub may_escape: bool,
    pub successors: Vec<DescriptorFlowEdge>,
}

pub struct DescriptorFlowEdge {
    pub target: DescriptorPosition,
    pub parameters: Vec<PlaceId>,
    pub bindings: Vec<(PlaceId, PlaceId)>,
}

impl DescriptorFlowEdge {
    fn forward(&self, source: PlaceId) -> impl Iterator<Item = PlaceId> + '_ {
        // Every actual belongs to the pre-edge state. A swap must not read a
        // destination after another binding has replaced that destination.
        (!self.parameters.contains(&source))
            .then_some(source)
            .into_iter()
            .chain(
                self.bindings.iter().filter_map(move |(actual, parameter)| {
                    (*actual == source).then_some(*parameter)
                }),
            )
    }
}

pub fn descriptor_live_positions(
    steps: &BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    birth: DescriptorPosition,
    descriptor: PlaceId,
) -> BTreeSet<DescriptorPosition> {
    // Materialize only reachable (position, name) links. Returning to the
    // formation kills its old result name, not independently carried aliases.
    type Occurrence = (DescriptorPosition, PlaceId);
    let mut pending = Vec::new();
    if let Some(step) = steps.get(&birth) {
        for edge in &step.successors {
            pending.extend(edge.forward(descriptor).map(|name| (edge.target, name)));
        }
    }
    let mut visited = BTreeSet::new();
    let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
    let mut observations = Vec::new();
    let mut escaped = Vec::new();
    while let Some(occurrence @ (position, name)) = pending.pop() {
        if !visited.insert(occurrence) {
            continue;
        }
        let Some(step) = steps.get(&position) else {
            continue;
        };
        if step.inputs.contains(&name) {
            observations.push(occurrence);
            if step.may_escape {
                escaped.push(position);
            }
        }
        let mut outputs = Vec::new();
        if !step.definitions.contains(&name) {
            outputs.push(name);
        }
        outputs.extend(
            step.derivations
                .iter()
                .filter_map(|(source, result)| (*source == name).then_some(*result)),
        );
        for edge in &step.successors {
            for output in &outputs {
                for destination in edge.forward(*output) {
                    let next = (edge.target, destination);
                    predecessors.entry(next).or_default().insert(occurrence);
                    pending.push(next);
                }
            }
        }
    }
    let mut observed = BTreeSet::new();
    let mut live = BTreeSet::new();
    while let Some(occurrence @ (position, _)) = observations.pop() {
        if !observed.insert(occurrence) {
            continue;
        }
        live.insert(position);
        observations.extend(predecessors.get(&occurrence).into_iter().flatten().copied());
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
    let Some(formation) = steps.get(&birth) else {
        return BTreeMap::new();
    };
    if !formation.definitions.contains(&descriptor) {
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
    // Start with no authority, including around downstream cycles. Iteration
    // can add only names justified by every predecessor; it never seeds a
    // guessed loop invariant or drops an unvisited arm of a diamond.
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
                        let mut after = before
                            .iter()
                            .filter(|place| !step.definitions.contains(place))
                            .copied()
                            .collect::<BTreeSet<_>>();
                        for result in &step.definitions {
                            let sources = step
                                .authority_derivations
                                .iter()
                                .filter(|(_, destination)| destination == result)
                                .map(|(source, _)| source)
                                .collect::<Vec<_>>();
                            if !sources.is_empty()
                                && sources.iter().all(|source| before.contains(source))
                            {
                                after.insert(*result);
                            }
                        }
                        if *predecessor == birth {
                            after.insert(descriptor);
                        }
                        let mut carried = after
                            .iter()
                            .filter(|place| !edge.parameters.contains(place))
                            .copied()
                            .collect::<BTreeSet<_>>();
                        for parameter in &edge.parameters {
                            let mut bindings = edge
                                .bindings
                                .iter()
                                .filter(|(_, target)| target == parameter);
                            if let Some((source, _)) = bindings.next()
                                && bindings.next().is_none()
                                && after.contains(source)
                            {
                                carried.insert(*parameter);
                            }
                        }
                        carried
                    });
            let mut current = incoming.next().unwrap_or_default();
            for other in incoming {
                current.retain(|place| other.contains(place));
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

#[cfg(test)]
mod tests;
