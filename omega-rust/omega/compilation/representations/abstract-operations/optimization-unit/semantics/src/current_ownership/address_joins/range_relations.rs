//! Current scalar ordering at a storage access. Facts come from executable
//! comparisons and selected edges, never from an unchecked obligation index.
//! Captured selectors additionally require unchanged current values: a new
//! value with the same SSA name cannot retarget an already formed loan.

use super::borrowed_regions::Position;
use abstract_operations::{AbstractOperation as O, ValueBinding};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType, ValueId};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Relation {
    left: ValueId,
    right: ValueId,
    strict: bool,
}

impl Relation {
    fn negated(self) -> Self {
        Self {
            left: self.right,
            right: self.left,
            strict: !self.strict,
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    relations: Vec<Relation>,
    comparisons: Vec<(ValueId, Relation)>,
    types: Vec<(ValueId, ScalarType)>,
}

impl State {
    fn kill(&mut self, values: &[ValueId]) {
        self.relations.retain(|relation| {
            !values.contains(&relation.left) && !values.contains(&relation.right)
        });
        self.comparisons.retain(|(value, relation)| {
            !values.contains(value)
                && !values.contains(&relation.left)
                && !values.contains(&relation.right)
        });
        self.types.retain(|(value, _)| !values.contains(value));
    }

    fn intersect(&mut self, other: &Self) {
        self.relations
            .retain(|relation| other.relations.contains(relation));
        self.comparisons
            .retain(|comparison| other.comparisons.contains(comparison));
        self.types.retain(|value| other.types.contains(value));
    }

    fn is_index(&self, value: ValueId) -> bool {
        self.types.contains(&(
            value,
            ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type"),
            ),
        ))
    }

    fn after(&self, operation: &O, position: Position) -> Self {
        let comparisons = match operation {
            O::IntegerLessThan {
                result,
                left,
                right,
                ..
            }
            | O::IntegerLessOrEqual {
                result,
                left,
                right,
                ..
            } if self.is_index(*left) && self.is_index(*right) => vec![(
                *result,
                Relation {
                    left: *left,
                    right: *right,
                    strict: matches!(operation, O::IntegerLessThan { .. }),
                },
            )],
            O::BooleanNot {
                result, operand, ..
            } => self
                .comparisons
                .iter()
                .filter(|(value, _)| value == operand)
                .map(|(_, relation)| (*result, relation.negated()))
                .collect(),
            _ => Vec::new(),
        };
        let definitions = crate::unit_validation::derived_metadata::expected_definitions(
            operation, position.0, position.1,
        );
        let mut result = self.clone();
        let defined = definitions
            .iter()
            .map(|definition| definition.value)
            .collect::<Vec<_>>();
        result.kill(&defined);
        result.types.extend(
            definitions
                .iter()
                .map(|definition| (definition.value, definition.scalar_type)),
        );
        for (value, relation) in comparisons {
            if !defined.contains(&relation.left) && !defined.contains(&relation.right) {
                result.comparisons.push((value, relation));
            }
        }
        result
    }

    fn across(&self, edge: &FlowEdge) -> Self {
        let mut source = self.clone();
        if let Some((condition, selected)) = edge.condition {
            // A transported comparison can describe several still-current
            // aliases. Retain each equivalent relation: the next edge may
            // carry a new parameter alias rather than the original name.
            for (_, relation) in source
                .comparisons
                .iter()
                .filter(|(value, _)| *value == condition)
            {
                let relation = if selected {
                    *relation
                } else {
                    relation.negated()
                };
                if !source.relations.contains(&relation) {
                    source.relations.push(relation);
                }
            }
        }
        if !edge.enters_block {
            return source;
        }
        // Validate the complete simultaneous assignment before using any part.
        // In particular a missing binding cannot preserve the old parameter.
        if edge.bindings.len() != edge.parameters.len()
            || edge.parameters.iter().any(|(parameter, scalar_type)| {
                let bindings = edge
                    .bindings
                    .iter()
                    .filter(|binding| binding.parameter == *parameter)
                    .collect::<Vec<_>>();
                bindings.len() != 1
                    || bindings[0].scalar_type != *scalar_type
                    || !source.types.contains(&(bindings[0].argument, *scalar_type))
            })
        {
            return Self::default();
        }
        let aliases = |value| {
            (!edge
                .parameters
                .iter()
                .any(|(parameter, _)| *parameter == value))
            .then_some(value)
            .into_iter()
            .chain(edge.bindings.iter().filter_map(move |binding| {
                (binding.argument == value).then_some(binding.parameter)
            }))
            .collect::<Vec<_>>()
        };
        let mut result = Self::default();
        for (value, scalar_type) in &source.types {
            for alias in aliases(*value) {
                result.types.push((alias, *scalar_type));
            }
        }
        for relation in &source.relations {
            for left in aliases(relation.left) {
                for right in aliases(relation.right) {
                    result.relations.push(Relation {
                        left,
                        right,
                        strict: relation.strict,
                    });
                }
            }
        }
        for (value, relation) in &source.comparisons {
            for alias in aliases(*value) {
                for left in aliases(relation.left) {
                    for right in aliases(relation.right) {
                        result.comparisons.push((
                            alias,
                            Relation {
                                left,
                                right,
                                strict: relation.strict,
                            },
                        ));
                    }
                }
            }
        }
        result.relations.sort_unstable();
        result.relations.dedup();
        result.comparisons.sort_unstable();
        result.comparisons.dedup();
        result.types.sort_unstable();
        result.types.dedup();
        result
    }
}

struct FlowEdge {
    target: usize,
    enters_block: bool,
    parameters: Vec<(ValueId, ScalarType)>,
    bindings: Vec<ValueBinding>,
    condition: Option<(ValueId, bool)>,
}

struct Step {
    position: Position,
    definitions: Vec<ValueId>,
    successors: Vec<FlowEdge>,
}

pub(super) struct RangeRelations {
    steps: Vec<Step>,
    states: Vec<Option<State>>,
}

impl RangeRelations {
    pub(super) fn new(function: &PsiOptimizationFunction) -> Self {
        let positions = function
            .blocks
            .iter()
            .flat_map(|block| {
                (0..block.nodes.len())
                    .filter_map(|index| u32::try_from(index).ok().map(|index| (block.id, index)))
            })
            .collect::<Vec<_>>();
        let mut steps = Vec::with_capacity(positions.len());
        for position in &positions {
            let block = function
                .blocks
                .iter()
                .find(|block| block.id == position.0)
                .expect("position comes from block");
            let operation = &block.nodes[position.1 as usize].operation;
            let edge = |target: BlockId, bindings: &[ValueBinding], condition| {
                let target_index = positions
                    .iter()
                    .position(|position| *position == (target, 0))?;
                let block = function.blocks.iter().find(|block| block.id == target)?;
                Some(FlowEdge {
                    target: target_index,
                    enters_block: true,
                    parameters: block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.value, parameter.scalar_type))
                        .collect(),
                    bindings: bindings.to_vec(),
                    condition,
                })
            };
            let successors = match operation {
                O::Jump {
                    target, bindings, ..
                } => edge(*target, bindings, None).into_iter().collect(),
                O::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => [
                    edge(
                        when_true.target,
                        &when_true.bindings,
                        Some((*condition, true)),
                    ),
                    edge(
                        when_false.target,
                        &when_false.bindings,
                        Some((*condition, false)),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
                O::StructuralCase { cases, .. } => cases
                    .iter()
                    .filter_map(|case| edge(case.target, &[], None))
                    .collect(),
                O::Return { .. }
                | O::ReturnUnit { .. }
                | O::ReturnStructural { .. }
                | O::Crash { .. } => Vec::new(),
                _ => positions
                    .iter()
                    .position(|next| *next == (position.0, position.1 + 1))
                    .map(|target| FlowEdge {
                        target,
                        enters_block: false,
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                        condition: None,
                    })
                    .into_iter()
                    .collect(),
            };
            steps.push(Step {
                position: *position,
                definitions: crate::unit_validation::derived_metadata::expected_definitions(
                    operation, position.0, position.1,
                )
                .iter()
                .map(|definition| definition.value)
                .collect(),
                successors,
            });
        }
        let mut result = Self {
            states: vec![None; steps.len()],
            steps,
        };
        let Some(entry) = positions
            .iter()
            .position(|position| *position == (function.entry, 0))
        else {
            return result;
        };
        let mut reachable = vec![false; positions.len()];
        let mut pending = vec![entry];
        while let Some(index) = pending.pop() {
            if std::mem::replace(&mut reachable[index], true) {
                continue;
            }
            pending.extend(
                result.steps[index]
                    .successors
                    .iter()
                    .map(|edge| edge.target),
            );
        }
        let mut remaining = vec![0usize; positions.len()];
        for (_, step) in result
            .steps
            .iter()
            .enumerate()
            .filter(|(index, _)| reachable[*index])
        {
            for edge in &step.successors {
                remaining[edge.target] += 1;
            }
        }
        // No guessed loop invariant: cyclic points and their unresolved
        // successors stay unknown. Acyclic paths use every reachable arrival.
        let mut queue = VecDeque::new();
        let mut arrivals = vec![None::<State>; positions.len()];
        arrivals[entry] = Some(State {
            types: function
                .parameters
                .iter()
                .map(|parameter| (parameter.value, parameter.scalar_type))
                .collect(),
            ..State::default()
        });
        if remaining[entry] == 0 {
            queue.push_back(entry);
        }
        while let Some(index) = queue.pop_front() {
            let state = arrivals[index].take().unwrap_or_default();
            let position = positions[index];
            let operation = &function
                .blocks
                .iter()
                .find(|block| block.id == position.0)
                .expect("known block")
                .nodes[position.1 as usize]
                .operation;
            let output = state.after(operation, position);
            result.states[index] = Some(state);
            for edge in &result.steps[index].successors {
                let arrival = output.across(edge);
                if let Some(current) = &mut arrivals[edge.target] {
                    current.intersect(&arrival);
                } else {
                    arrivals[edge.target] = Some(arrival);
                }
                remaining[edge.target] -= 1;
                if remaining[edge.target] == 0 {
                    queue.push_back(edge.target);
                }
            }
        }
        result
    }

    pub(super) fn proves_less_than(
        &self,
        position: Position,
        left: ValueId,
        right: ValueId,
    ) -> bool {
        self.proves(position, left, right, true)
    }

    pub(super) fn proves_less_or_equal(
        &self,
        position: Position,
        left: ValueId,
        right: ValueId,
    ) -> bool {
        self.proves(position, left, right, false)
    }

    fn proves(&self, position: Position, left: ValueId, right: ValueId, strict: bool) -> bool {
        let Some(state) = self
            .steps
            .iter()
            .position(|step| step.position == position)
            .and_then(|index| self.states[index].as_ref())
        else {
            return false;
        };
        if !state.is_index(left) || !state.is_index(right) {
            return false;
        }
        let mut pending = vec![(left, false)];
        let mut visited = Vec::new();
        while let Some((value, already_strict)) = pending.pop() {
            if value == right && (!strict || already_strict) {
                return true;
            }
            if visited.contains(&(value, already_strict)) {
                continue;
            }
            visited.push((value, already_strict));
            pending.extend(
                state
                    .relations
                    .iter()
                    .filter(|relation| relation.left == value)
                    .map(|relation| (relation.right, already_strict || relation.strict)),
            );
        }
        false
    }

    /// The formation precedes this access on every path, and each captured
    /// name still denotes its formation-time value. Edge assignments count as
    /// redefinitions even when another path happens to bind an equal value.
    pub(super) fn values_unchanged_since(
        &self,
        birth: Position,
        current: Position,
        values: &[ValueId],
    ) -> bool {
        let Some(birth) = self.steps.iter().position(|step| step.position == birth) else {
            return false;
        };
        let Some(current) = self.steps.iter().position(|step| step.position == current) else {
            return false;
        };
        if self.states[birth].is_none() || self.states[current].is_none() {
            return false;
        }
        let mut pending = vec![current];
        let mut visited = Vec::new();
        while let Some(index) = pending.pop() {
            if index == birth || visited.contains(&index) {
                continue;
            }
            visited.push(index);
            let mut found = false;
            for (predecessor, step) in self.steps.iter().enumerate() {
                if self.states[predecessor].is_none() {
                    continue;
                }
                for edge in step.successors.iter().filter(|edge| edge.target == index) {
                    found = true;
                    if step.definitions.iter().any(|value| values.contains(value))
                        || edge
                            .parameters
                            .iter()
                            .any(|(value, _)| values.contains(value))
                    {
                        return false;
                    }
                    pending.push(predecessor);
                }
            }
            if !found {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "range_relations/tests.rs"]
mod tests;
