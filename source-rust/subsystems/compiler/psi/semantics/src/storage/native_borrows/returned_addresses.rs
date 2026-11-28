//! Which scalar formals can contribute to a normally returned address.
//! Both the Psi and selected-operation readers supply their own definitions.
//! Only the scalar slice feeding a return is needed for this question.
use super::{AddressDefinition, trace_local};
use arena::IdSet;
use semantic_vocabulary::{MachineId, ValueId};
use std::collections::{BTreeSet, VecDeque};

pub struct AddressReturnBody {
    formals: Vec<ValueId>,
    results: Vec<ValueId>,
    definitions: Vec<(ValueId, AddressDefinition<()>)>,
}

impl AddressReturnBody {
    pub fn new<S>(
        formals: Vec<ValueId>,
        results: Vec<ValueId>,
        mut definition: impl FnMut(ValueId) -> AddressDefinition<S>,
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
                AddressDefinition::Call(machine, arguments) => {
                    pending.extend(&arguments);
                    AddressDefinition::Call(machine, arguments)
                }
                // A callee-local storage coordinate cannot name one of the
                // caller's scalar formals. It remains an independent route.
                AddressDefinition::Storage(..) | AddressDefinition::Opaque => {
                    AddressDefinition::Opaque
                }
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
                |callee, actuals| ReturnInputs::substitute(summaries.summary(callee), actuals),
            );
            opaque |= origin.opaque;
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
            exact: exact && !opaque && parameters.len() == 1,
            opaque,
            parameters,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ReturnInputs {
    parameters: BTreeSet<usize>,
    exact: bool,
    opaque: bool,
}

pub(super) struct ReturnedActuals {
    pub values: Vec<ValueId>,
    pub exact: bool,
    pub opaque: bool,
}

impl ReturnInputs {
    fn substitute(summary: Option<&Self>, actuals: &[ValueId]) -> ReturnedActuals {
        let values = summary.and_then(|s| {
            s.parameters
                .iter()
                .map(|&index| actuals.get(index).copied())
                .collect::<Option<Vec<_>>>()
        });
        match (summary, values) {
            (Some(s), Some(values)) => ReturnedActuals {
                values,
                exact: s.exact,
                opaque: s.opaque,
            },
            _ => ReturnedActuals {
                values: vec![],
                exact: false,
                opaque: true,
            },
        }
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
        callee: MachineId,
        actuals: &[ValueId],
        body: &mut impl FnMut(MachineId) -> Option<AddressReturnBody>,
    ) -> ReturnedActuals {
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
                    if let AddressDefinition::Call(target, _) = step {
                        edges.push((*target, id));
                        discover.push(*target);
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
        ReturnInputs::substitute(self.summary(callee), actuals)
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
        let reached = returns.actuals(machine, &[left, right], &mut |_| {
            Some(AddressReturnBody::new(
                vec![left, right],
                vec![result],
                |value| {
                    if value == result {
                        AddressDefinition::<()>::Incoming(vec![Some(left), Some(recursive)])
                    } else if value == recursive {
                        AddressDefinition::Call(machine, vec![right, left])
                    } else {
                        AddressDefinition::Parameter
                    }
                },
            ))
        });
        assert_eq!(reached.values, [left, right]);
        assert!(!reached.exact);
    }
}
