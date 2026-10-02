//! Scalar ordering reconstructed from executable comparisons and selected edges.
//! The solver owns only supplied analysis scratch: each IR adapter reconstructs
//! its exact graph, definitions and effects. Cycles remain unknown, joins retain
//! all-path facts, and captured selectors require unchanged formation values.
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, ScalarType, ValueId};
use std::collections::VecDeque;

/// A current executable coordinate, including terminator steps supplied by adapters.
pub type Position = (BlockId, u32);

/// Reconstructed executable scalar behavior; this is analysis scratch, not an IR.
#[derive(Clone, Copy, Debug, Default)]
pub enum Effect {
    #[default]
    None,
    Compare {
        result: ValueId,
        left: ValueId,
        right: ValueId,
        strict: bool,
    },
    Not {
        result: ValueId,
        operand: ValueId,
    },
    UnsignedLiteral {
        result: ValueId,
        value: u128,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub parameter: ValueId,
    pub argument: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub target: usize,
    pub enters_block: bool,
    pub parameters: Vec<(ValueId, ScalarType)>,
    pub bindings: Vec<Binding>,
    pub condition: Option<(ValueId, bool)>,
}

#[derive(Clone, Debug)]
pub struct Step {
    pub position: Position,
    pub definitions: Vec<(ValueId, ScalarType)>,
    pub effect: Effect,
    pub successors: Vec<Edge>,
}

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
    literals: Vec<(ValueId, u128)>,
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
        self.literals.retain(|(value, _)| !values.contains(value));
    }

    fn intersect(&mut self, other: &Self) {
        self.relations
            .retain(|relation| other.relations.contains(relation));
        self.comparisons
            .retain(|comparison| other.comparisons.contains(comparison));
        self.types.retain(|value| other.types.contains(value));
        self.literals.retain(|value| other.literals.contains(value));
    }

    fn is_index(&self, value: ValueId) -> bool {
        self.types.contains(&(
            value,
            ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is a valid integer type"),
            ),
        ))
    }

    fn after(&self, step: &Step) -> Self {
        let result_is = |result, scalar_type| {
            step.definitions
                .iter()
                .filter(|(value, _)| *value == result)
                .count()
                == 1
                && step.definitions.contains(&(result, scalar_type))
        };
        let comparisons = match step.effect {
            Effect::Compare {
                result,
                left,
                right,
                strict,
            } if self.is_index(left)
                && self.is_index(right)
                && result_is(result, ScalarType::Boolean) =>
            {
                vec![(
                    result,
                    Relation {
                        left,
                        right,
                        strict,
                    },
                )]
            }
            Effect::Not { result, operand }
                if self.types.contains(&(operand, ScalarType::Boolean))
                    && result_is(result, ScalarType::Boolean) =>
            {
                self.comparisons
                    .iter()
                    .filter(|(value, _)| *value == operand)
                    .map(|(_, relation)| (result, relation.negated()))
                    .collect()
            }
            _ => Vec::new(),
        };
        let mut result = self.clone();
        let defined = step
            .definitions
            .iter()
            .map(|(value, _)| *value)
            .collect::<Vec<_>>();
        result.kill(&defined);
        result.types.extend_from_slice(&step.definitions);
        for (value, relation) in comparisons {
            if !defined.contains(&relation.left) && !defined.contains(&relation.right) {
                result.comparisons.push((value, relation));
            }
        }
        if let Effect::UnsignedLiteral {
            result: value,
            value: literal,
        } = step.effect
            && literal <= u128::from(u64::MAX)
            && result.is_index(value)
            && step
                .definitions
                .iter()
                .filter(|(defined, _)| *defined == value)
                .count()
                == 1
        {
            result.literals.push((value, literal));
        }
        result
    }

    fn across(&self, edge: &Edge) -> Self {
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
        for (value, literal) in &source.literals {
            for alias in aliases(*value) {
                result.literals.push((alias, *literal));
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
        result.literals.sort_unstable();
        result.literals.dedup();
        result
    }
}

/// All-path ordering and captured-value stability over a supplied current graph.
/// Adapters reconstruct definitions, effects and edges from their validated IR;
/// no serialized fact or obligation lookup enters this analysis.
pub struct ScalarOrdering {
    steps: Vec<Step>,
    states: Vec<Option<State>>,
}

impl ScalarOrdering {
    pub fn new(steps: Vec<Step>, entry: Position, parameters: Vec<(ValueId, ScalarType)>) -> Self {
        let positions = steps.iter().map(|step| step.position).collect::<Vec<_>>();
        let mut result = Self {
            states: vec![None; steps.len()],
            steps,
        };
        if result.steps.iter().any(|step| {
            step.successors
                .iter()
                .any(|edge| edge.target >= result.steps.len())
        }) || positions
            .iter()
            .enumerate()
            .any(|(index, position)| positions[..index].contains(position))
        {
            return result;
        }
        let Some(entry) = positions.iter().position(|position| *position == entry) else {
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
            types: parameters,
            ..State::default()
        });
        if remaining[entry] == 0 {
            queue.push_back(entry);
        }
        while let Some(index) = queue.pop_front() {
            let state = arrivals[index].take().unwrap_or_default();
            let output = state.after(&result.steps[index]);
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

    pub fn proves_less_than(&self, position: Position, left: ValueId, right: ValueId) -> bool {
        self.proves(position, left, right, true)
    }

    pub fn proves_less_or_equal(&self, position: Position, left: ValueId, right: ValueId) -> bool {
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
            if let Some((_, literal)) = state.literals.iter().find(|(known, _)| *known == value) {
                pending.extend(
                    state
                        .literals
                        .iter()
                        .filter(|(_, other)| literal <= other)
                        .map(|(other, other_literal)| {
                            (*other, already_strict || literal < other_literal)
                        }),
                );
            }
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
    pub fn values_unchanged_since(
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
                    if step
                        .definitions
                        .iter()
                        .any(|(value, _)| values.contains(value))
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
mod tests;
