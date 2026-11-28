//! Which inputs and invocation-owned homes can contribute to a normally returned address.
//! Both the Psi and selected-operation readers supply their own definitions.
//! Only the scalar slice feeding a return is needed for this question.
use super::returned_storage::StorageInput;
use super::{AddressCall, AddressDefinition, Position, ReturnedStorage, trace_local};
use arena::IdSet;
use semantic_vocabulary::{MachineId, ValueId};
use std::collections::{BTreeSet, VecDeque};
use terminal_psi::{StructuralArgument, StructuralPathSegment};

pub struct AddressReturnBody {
    formals: Vec<ValueId>,
    results: Vec<ValueId>,
    definitions: Vec<(ValueId, AddressDefinition<ReturnedStorage>)>,
}

impl AddressReturnBody {
    pub fn new(
        formals: Vec<ValueId>,
        results: Vec<ValueId>,
        mut definition: impl FnMut(ValueId) -> AddressDefinition<StructuralArgument>,
        mut storage: impl FnMut(&StructuralArgument, Position) -> ReturnedStorage,
    ) -> Self {
        let mut pending = results.clone();
        let mut visited = IdSet::new();
        let mut definitions = Vec::new();
        while let Some(value) = pending.pop() {
            if !visited.insert(value) {
                continue;
            }
            let step = match definition(value) {
                AddressDefinition::Parameter => AddressDefinition::Parameter,
                AddressDefinition::Forward(input) => {
                    pending.push(input);
                    AddressDefinition::Forward(input)
                }
                AddressDefinition::Adjust(left, right) => {
                    pending.extend([left, right]);
                    AddressDefinition::Adjust(left, right)
                }
                AddressDefinition::Incoming(inputs) => {
                    pending.extend(inputs.iter().flatten());
                    AddressDefinition::Incoming(inputs)
                }
                AddressDefinition::Call(call) => {
                    pending.extend(&call.scalars);
                    AddressDefinition::Call(AddressCall {
                        callee: call.callee,
                        scalars: call.scalars,
                        storage: call
                            .storage
                            .iter()
                            .map(|source| storage(source, call.at))
                            .collect(),
                        at: call.at,
                    })
                }
                AddressDefinition::Storage(source, at) => {
                    AddressDefinition::Storage(storage(&source, at), at)
                }
                AddressDefinition::Opaque => AddressDefinition::Opaque,
            };
            definitions.push((value, step));
        }
        definitions.sort_unstable_by_key(|&(id, _)| id);
        Self {
            formals,
            results,
            definitions,
        }
    }

    fn inputs(&self, summaries: &ReturnSources) -> ReturnInputs {
        let mut parameters = BTreeSet::new();
        let mut exact = true;
        let mut storage = Vec::new();
        let mut expired = false;
        let mut opaque = self.results.is_empty();
        for &result in &self.results {
            let origin = trace_local(
                result,
                |id| {
                    self.definitions
                        .binary_search_by_key(&id, |&(key, _)| key)
                        .ok()
                        .map(|i| self.definitions[i].1.clone())
                        .unwrap_or(AddressDefinition::Opaque)
                },
                |call| {
                    ReturnInputs::substitute(
                        summaries.summary(call.callee),
                        call,
                        ReturnedStorage::project,
                    )
                },
            );
            opaque |= origin.opaque;
            expired |= origin.expired;
            for source in origin.sources {
                match source {
                    ReturnedStorage::Borrowed { ordinal, path } => {
                        exact &= StorageInput::join(&mut storage, ordinal, path)
                    }
                    ReturnedStorage::Local => expired = true,
                    ReturnedStorage::Opaque => opaque = true,
                }
            }
            exact &= origin.exact;
            for formal in origin.parameters {
                match self.formals.iter().position(|&id| id == formal) {
                    Some(index) => {
                        parameters.insert(index);
                    }
                    None => opaque = true,
                }
            }
        }
        ReturnInputs {
            exact: exact && !opaque && !expired && parameters.len() + storage.len() == 1,
            storage,
            expired,
            opaque,
            parameters,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ReturnInputs {
    storage: Vec<StorageInput>,
    expired: bool,
    parameters: BTreeSet<usize>,
    exact: bool,
    opaque: bool,
}

pub(super) struct ReturnedActuals<S> {
    pub values: Vec<ValueId>,
    pub storage: Vec<S>,
    pub exact: bool,
    pub opaque: bool,
    pub expired: bool,
}

impl ReturnInputs {
    fn substitute<S>(
        summary: Option<&Self>,
        call: &AddressCall<S>,
        project: impl Fn(&S, &[StructuralPathSegment]) -> S,
    ) -> ReturnedActuals<S> {
        let mut actuals = ReturnedActuals {
            values: Vec::new(),
            storage: Vec::new(),
            exact: false,
            opaque: true,
            expired: false,
        };
        if let Some(summary) = summary {
            actuals.exact = summary.exact;
            actuals.opaque = summary.opaque;
            actuals.expired = summary.expired;
            for &ordinal in &summary.parameters {
                match call.scalars.get(ordinal) {
                    Some(value) => actuals.values.push(*value),
                    None => {
                        actuals.opaque = true;
                        actuals.exact = false;
                    }
                }
            }
            for input in &summary.storage {
                match call.storage.get(input.ordinal) {
                    Some(source) => actuals.storage.push(project(source, &input.path)),
                    None => {
                        actuals.opaque = true;
                        actuals.exact = false;
                    }
                }
            }
        }
        actuals
    }
}

#[derive(Default)]
pub(super) struct ReturnSources {
    summaries: Vec<(MachineId, ReturnInputs)>,
}

impl ReturnSources {
    fn summary(&self, machine: MachineId) -> Option<&ReturnInputs> {
        let row = self
            .summaries
            .binary_search_by_key(&machine, |&(id, _)| id)
            .ok()?;
        Some(&self.summaries[row].1)
    }

    pub(super) fn actuals(
        &mut self,
        call: &AddressCall<StructuralArgument>,
        body: &mut impl FnMut(MachineId) -> Option<AddressReturnBody>,
    ) -> ReturnedActuals<StructuralArgument> {
        let callee = call.callee;
        if self.summary(callee).is_none() {
            let mut visited = IdSet::new();
            let mut bodies = Vec::new();
            let mut edges = Vec::new();
            let mut discover = vec![callee];
            while let Some(id) = discover.pop() {
                if !visited.insert(id) || self.summary(id).is_some() {
                    continue;
                }
                let Some(machine) = body(id) else {
                    continue;
                };
                for (_, step) in &machine.definitions {
                    if let AddressDefinition::Call(call) = step {
                        edges.push((call.callee, id));
                        discover.push(call.callee);
                    }
                }
                bodies.push((id, machine));
            }
            bodies.sort_unstable_by_key(|&(id, _)| id);
            edges.sort_unstable();
            edges.dedup();
            // Revisit callers when a return reveals another formal. Recursive
            // components start with an opaque alternative and retain every
            // known input they can contribute, including permuted arguments.
            let mut work: VecDeque<_> = (0..bodies.len()).collect();
            while let Some(row) = work.pop_front() {
                let (id, machine) = &bodies[row];
                let summary = machine.inputs(self);
                match self.summaries.binary_search_by_key(id, |&(key, _)| key) {
                    Ok(i) if self.summaries[i].1 == summary => continue,
                    Ok(i) => self.summaries[i].1 = summary,
                    Err(i) => self.summaries.insert(i, (*id, summary)),
                }
                let first = edges.partition_point(|(target, _)| target < id);
                for (_, caller) in edges[first..].iter().take_while(|(target, _)| target == id) {
                    if let Ok(row) = bodies.binary_search_by_key(caller, |&(key, _)| key) {
                        work.push_back(row);
                    }
                }
            }
        }
        ReturnInputs::substitute(self.summary(callee), call, |source, suffix| {
            let mut projected = source.clone();
            projected.path.extend_from_slice(suffix);
            projected
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_return_routes_keep_formals_from_later_iterations() {
        let left = ValueId::FIRST;
        let right = ValueId::new(2).unwrap();
        let recursive = ValueId::new(3).unwrap();
        let result = ValueId::new(4).unwrap();
        let machine = MachineId::new(1).unwrap();
        let mut returns = ReturnSources::default();
        let at = (semantic_vocabulary::BlockId::new(1).unwrap(), 0);
        let call = |scalars| AddressCall {
            callee: machine,
            scalars,
            storage: Vec::new(),
            at,
        };
        let reached = returns.actuals(&call(vec![left, right]), &mut |_| {
            Some(AddressReturnBody::new(
                vec![left, right],
                vec![result],
                |value| {
                    if value == result {
                        AddressDefinition::Incoming(vec![Some(left), Some(recursive)])
                    } else if value == recursive {
                        AddressDefinition::Call(call(vec![right, left]))
                    } else {
                        AddressDefinition::Parameter
                    }
                },
                |_, _| ReturnedStorage::Opaque,
            ))
        });
        assert_eq!(reached.values, [left, right]);
        assert!(!reached.exact);
    }
}
