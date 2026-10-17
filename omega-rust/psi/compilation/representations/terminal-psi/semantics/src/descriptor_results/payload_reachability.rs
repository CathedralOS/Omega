//! Necessary Boolean conditions for reaching a payload observation.
//!
//! Backward transport follows actual scalar edge bindings, not requested domain
//! annotations. Alternative routes intersect their necessary conditions; unknown
//! operations forget them. Only a contradictory literal/polarity removes an edge.
//! The forward payload analysis still intersects every remaining predecessor.
use super::*;
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType, ValueId};
use std::collections::VecDeque;

type Conditions = BTreeMap<ValueId, bool>;

#[cfg(test)]
mod tests;

pub(super) struct Reachability {
    positions: BTreeSet<DescriptorPosition>,
    edges: BTreeSet<(DescriptorPosition, usize)>,
}

impl Reachability {
    pub(super) fn contains(&self, position: DescriptorPosition) -> bool {
        self.positions.contains(&position)
    }

    pub(super) fn includes_edge(&self, position: DescriptorPosition, ordinal: usize) -> bool {
        self.edges.contains(&(position, ordinal))
    }
}

pub(super) fn predecessors(
    machine: &DescriptorMachine,
    query: DescriptorPosition,
) -> Option<Reachability> {
    QueryFlow {
        flow: &machine.flow,
        predicates: &machine.payload_edge_predicates,
        bindings: &machine.payload_scalar_bindings,
        parameters: &machine.payload_scalar_parameters,
        definitions: &machine.payload_scalar_definitions,
    }
    .predecessors(query)
}

struct QueryFlow<'flow> {
    flow: &'flow BTreeMap<DescriptorPosition, DescriptorFlowStep>,
    predicates: &'flow BTreeMap<DescriptorPosition, Vec<Option<Proposition>>>,
    bindings: &'flow BTreeMap<DescriptorPosition, Vec<Vec<(ValueId, ScalarTerm)>>>,
    parameters: &'flow BTreeMap<semantic_vocabulary::BlockId, Vec<ValueId>>,
    definitions: &'flow BTreeMap<DescriptorPosition, Vec<ValueId>>,
}

impl QueryFlow<'_> {
    fn predecessors(&self, query: DescriptorPosition) -> Option<Reachability> {
        self.flow.get(&query)?;
        let mut incoming = BTreeMap::<_, Vec<_>>::new();
        for (&position, step) in self.flow {
            for (ordinal, edge) in step.successors.iter().enumerate() {
                self.flow.get(&edge.target)?;
                incoming
                    .entry(edge.target)
                    .or_default()
                    .push((position, ordinal));
            }
        }
        let mut conditions = BTreeMap::from([(query, Conditions::new())]);
        let mut edges = BTreeSet::new();
        let mut pending = VecDeque::from([query]);
        let mut remaining = 65_536usize;
        while let Some(destination) = pending.pop_front() {
            remaining = remaining.checked_sub(1)?;
            for &(position, ordinal) in incoming.get(&destination).into_iter().flatten() {
                let Some(arrival) = self.before_edge(
                    position,
                    ordinal,
                    destination,
                    conditions.get(&destination)?,
                ) else {
                    continue;
                };
                edges.insert((position, ordinal));
                let changed = if let Some(previous) = conditions.get_mut(&position) {
                    let count = previous.len();
                    previous.retain(|value, polarity| arrival.get(value) == Some(polarity));
                    previous.len() != count
                } else {
                    conditions.insert(position, arrival);
                    true
                };
                if changed && !pending.contains(&position) {
                    pending.push_back(position);
                }
            }
        }
        Some(Reachability {
            positions: conditions.into_keys().collect(),
            edges,
        })
    }

    fn before_edge(
        &self,
        position: DescriptorPosition,
        ordinal: usize,
        destination: DescriptorPosition,
        after: &Conditions,
    ) -> Option<Conditions> {
        let mut before = after.clone();
        if destination.1 == 0 {
            // Remove all new instances before substituting any old actual.
            // This also handles self-loop and swapped parameter bindings.
            if let Some(parameters) = self.parameters.get(&destination.0) {
                before.retain(|value, _| !parameters.contains(value));
            }
            if let Some(bindings) = self
                .bindings
                .get(&position)
                .and_then(|edges| edges.get(ordinal))
            {
                for (parameter, actual) in bindings {
                    if let Some(&polarity) = after.get(parameter) {
                        require(&mut before, actual, polarity)?;
                    }
                }
            }
        }
        if let Some(Some(Proposition::Equal(left, right))) = self
            .predicates
            .get(&position)
            .and_then(|edges| edges.get(ordinal))
        {
            match (left, right) {
                (ScalarTerm::Boolean(polarity), term) | (term, ScalarTerm::Boolean(polarity)) => {
                    require(&mut before, term, *polarity)?
                }
                _ => {}
            }
        }
        // A definition starts a new dynamic instance. Its future constraint
        // cannot constrain the previous iteration's identically numbered value.
        if let Some(definitions) = self.definitions.get(&position) {
            before.retain(|value, _| !definitions.contains(value));
        }
        Some(before)
    }
}

fn require(conditions: &mut Conditions, mut term: &ScalarTerm, mut polarity: bool) -> Option<()> {
    for _ in 0..64 {
        match term {
            ScalarTerm::Boolean(value) => return (*value == polarity).then_some(()),
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Boolean,
            } => {
                return match conditions.insert(*id, polarity) {
                    Some(previous) if previous != polarity => None,
                    _ => Some(()),
                };
            }
            ScalarTerm::BooleanNot { operand } => {
                term = operand;
                polarity = !polarity;
            }
            // No inference from unknown terms, including mutable observations.
            _ => return Some(()),
        }
    }
    Some(())
}
