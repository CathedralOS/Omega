//! Scalar ordering from executable comparisons, selected edges and authenticated contracts.
//! The solver owns only supplied analysis scratch: each IR adapter reconstructs
//! its exact graph, definitions and effects. Cycles remain unknown, joins retain
//! all-path facts, and captured selectors require unchanged formation values.
use semantic_vocabulary::{BlockId, IntegerSign, IntegerType, Proposition, ScalarType, ValueId};

mod contracts;
pub use contracts::instantiate_contract_ordering;
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
pub(crate) enum Term {
    Value(ValueId),
    Unsigned(u64),
}

impl Term {
    fn mentions(self, values: &[ValueId]) -> bool {
        matches!(self, Self::Value(value) if values.contains(&value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Relation {
    left: Term,
    right: Term,
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
        self.relations
            .retain(|relation| !relation.left.mentions(values) && !relation.right.mentions(values));
        self.comparisons.retain(|(value, relation)| {
            !values.contains(value)
                && !relation.left.mentions(values)
                && !relation.right.mentions(values)
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

    fn assume(&mut self, propositions: &[Proposition]) {
        for relation in contracts::relations(propositions) {
            let valid = |term| match term {
                Term::Value(value) => self.is_index(value),
                Term::Unsigned(_) => true,
            };
            if valid(relation.left) && valid(relation.right) {
                self.relations.push(relation);
            }
        }
        self.relations.sort_unstable();
        self.relations.dedup();
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
                        left: Term::Value(left),
                        right: Term::Value(right),
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
            if !relation.left.mentions(&defined) && !relation.right.mentions(&defined) {
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
        let term_aliases = |term| match term {
            Term::Value(value) => aliases(value).into_iter().map(Term::Value).collect(),
            Term::Unsigned(_) => vec![term],
        };
        for relation in &source.relations {
            for left in term_aliases(relation.left) {
                for right in term_aliases(relation.right) {
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
                for left in term_aliases(relation.left) {
                    for right in term_aliases(relation.right) {
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
/// adapters authenticate contracts independently of incidental operation identities.
pub struct ScalarOrdering {
    steps: Vec<Step>,
    states: Vec<Option<State>>,
}

impl ScalarOrdering {
    pub fn new(steps: Vec<Step>, entry: Position, parameters: Vec<(ValueId, ScalarType)>) -> Self {
        Self::with_contracts(steps, entry, parameters, &[], Vec::new())
    }

    /// Entry requirements hold for incoming formals. Normal-return guarantees
    /// hold only after their exact current call's result definitions. Adapters
    /// must reconstruct those applications from independently checked contracts;
    /// this solver neither grants contract authority nor accepts storage facts.
    pub fn with_contracts(
        steps: Vec<Step>,
        entry: Position,
        parameters: Vec<(ValueId, ScalarType)>,
        entry_requirements: &[Proposition],
        normal_returns: Vec<(Position, Vec<Proposition>)>,
    ) -> Self {
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
        let mut initial = State {
            types: parameters,
            ..State::default()
        };
        initial.assume(entry_requirements);
        arrivals[entry] = Some(initial);
        if remaining[entry] == 0 {
            queue.push_back(entry);
        }
        while let Some(index) = queue.pop_front() {
            let state = arrivals[index].take().unwrap_or_default();
            let mut output = state.after(&result.steps[index]);
            for (_, guarantees) in normal_returns
                .iter()
                .filter(|(position, _)| *position == result.steps[index].position)
            {
                output.assume(guarantees);
            }
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
        self.proves(position, Term::Value(left), Term::Value(right), true)
    }

    pub fn proves_less_or_equal(&self, position: Position, left: ValueId, right: ValueId) -> bool {
        self.proves(position, Term::Value(left), Term::Value(right), false)
    }

    pub(crate) fn proves(&self, position: Position, left: Term, right: Term, strict: bool) -> bool {
        let Some(state) = self
            .steps
            .iter()
            .position(|step| step.position == position)
            .and_then(|index| self.states[index].as_ref())
        else {
            return false;
        };
        let valid = |term| match term {
            Term::Value(value) => state.is_index(value),
            Term::Unsigned(_) => true,
        };
        if !valid(left) || !valid(right) {
            return false;
        }
        let mut relations = state.relations.clone();
        for (value, literal) in &state.literals {
            let Ok(literal) = u64::try_from(*literal) else {
                continue;
            };
            for (left, right) in [
                (Term::Value(*value), Term::Unsigned(literal)),
                (Term::Unsigned(literal), Term::Value(*value)),
            ] {
                relations.push(Relation {
                    left,
                    right,
                    strict: false,
                });
            }
        }
        let constants = relations
            .iter()
            .flat_map(|relation| [relation.left, relation.right])
            .chain([left, right])
            .filter_map(|term| match term {
                Term::Unsigned(value) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut pending = vec![(left, false)];
        let mut visited = Vec::new();
        while let Some((term, already_strict)) = pending.pop() {
            if term == right && (!strict || already_strict) {
                return true;
            }
            if visited.contains(&(term, already_strict)) {
                continue;
            }
            visited.push((term, already_strict));
            if let Term::Unsigned(literal) = term {
                pending.extend(
                    constants
                        .iter()
                        .filter(|other| literal <= **other)
                        .map(|other| (Term::Unsigned(*other), already_strict || literal < *other)),
                );
            }
            pending.extend(
                relations
                    .iter()
                    .filter(|relation| relation.left == term)
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
