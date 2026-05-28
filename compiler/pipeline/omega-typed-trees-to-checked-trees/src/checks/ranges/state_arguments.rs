mod calls;
mod expressions;

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::arrays::fixed_array_type_length;
use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::seed_machine_requires;

use self::calls::collect_state_argument_facts_for_call;
use self::expressions::collect_state_argument_facts_from_expression;

#[derive(Clone, Debug, Default)]
pub(super) struct StateArgumentFacts {
    state: SymbolHandle,
    parameters: Vec<ParameterFacts>,
}

#[derive(Clone, Debug)]
struct ParameterFacts {
    symbol: SymbolHandle,
    name: String,
    is_self: bool,
    length: MergedFact<usize>,
    integer: MergedFact<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MergedFact<T> {
    Unseen,
    Known(T),
    Conflicting,
}

impl<T> Default for MergedFact<T> {
    fn default() -> Self {
        Self::Unseen
    }
}

impl<T: Copy + Eq> MergedFact<T> {
    fn merge(&mut self, value: Option<T>) {
        match (*self, value) {
            (Self::Unseen, Some(value)) => *self = Self::Known(value),
            (Self::Unseen | Self::Known(_), None) => *self = Self::Conflicting,
            (Self::Known(existing), Some(value)) if existing == value => {}
            (Self::Known(_), Some(_)) | (Self::Conflicting, _) => *self = Self::Conflicting,
        }
    }

    fn get(self) -> Option<T> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unseen | Self::Conflicting => None,
        }
    }
}

pub(super) fn collect_state_argument_facts(
    program: &omega_typed_trees::TypedTrees,
    field_lengths: &[(SymbolHandle, String, usize)],
    machine: &Machine,
) -> Vec<StateArgumentFacts> {
    let mut collected = Vec::new();
    for state in program.machine_states(machine) {
        let mut facts = RangeFacts::new(field_lengths);
        seed_machine_requires(program, &mut facts, machine);
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_state_argument_facts_from_statement(
                program,
                machine,
                &mut facts,
                statement,
                &mut collected,
            );
        }
    }
    collected
}

pub(super) fn seed_state_argument_facts(
    facts: &mut RangeFacts<'_>,
    state: &State,
    collected: &[StateArgumentFacts],
) {
    let Some(state_facts) = collected.iter().find(|entry| entry.state == state.symbol) else {
        return;
    };

    for parameter in &state_facts.parameters {
        facts.define_local(
            parameter.symbol,
            parameter.name.clone(),
            parameter.length.get(),
            parameter.integer.get(),
        );
    }
}

fn collect_state_argument_facts_from_statement(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    collected: &mut Vec<StateArgumentFacts>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
            }
        }
        StatementNode::Call(call) => {
            collect_state_argument_facts_for_call(
                program,
                machine,
                facts,
                call.target_symbol,
                Some(&call.target),
                program.statement_table.expression_handles(call.arguments),
                collected,
            );
        }
        StatementNode::Expression(expression) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                *expression,
                collected,
            );
        }
        StatementNode::LocalData(local) => {
            let length = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference));
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
        }
        StatementNode::Transition(transition) => {
            collect_state_argument_facts_from_target(
                program,
                machine,
                facts,
                transition.target,
                collected,
            );
            collect_state_argument_facts_from_target(
                program,
                machine,
                facts,
                transition.continuation,
                collected,
            );
        }
    }
}

fn collect_state_argument_facts_from_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    if !target.is_valid() {
        return;
    }

    let TransitionTargetNode::Named { path, arguments } =
        program.statement_table.transition_target(target)
    else {
        return;
    };
    let Some(target_state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == path.symbol)
    else {
        return;
    };

    collect_state_argument_facts_for_call(
        program,
        machine,
        facts,
        target_state.symbol,
        Some(&target_state.name),
        program.statement_table.expression_handles(*arguments),
        collected,
    );
}
