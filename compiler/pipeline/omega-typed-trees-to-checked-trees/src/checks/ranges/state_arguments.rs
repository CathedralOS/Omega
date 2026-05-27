use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::{RangeFacts, fixed_array_type_length};
use super::seed_machine_requires;

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

fn collect_state_argument_facts_from_expression(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_state_argument_facts_from_expression(
                    program, machine, facts, *value, collected,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                binary.left,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                binary.right,
                collected,
            );
        }
        ExpressionNode::Call(call) => {
            collect_state_argument_facts_for_call(
                program,
                machine,
                facts,
                call.target_symbol,
                Some(&call.target),
                program.expression_table.expression_handles(call.arguments),
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                call.receiver,
                collected,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_state_argument_facts_from_expression(
                    program, machine, facts, *argument, collected,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_state_argument_facts_from_expression(
                program, machine, facts, cast.value, collected,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                indexed.collection,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                indexed.index,
                collected,
            );
        }
        ExpressionNode::Member(member) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                member.receiver,
                collected,
            );
        }
        ExpressionNode::Mutable(inner) => {
            collect_state_argument_facts_from_expression(
                program, machine, facts, *inner, collected,
            );
        }
        ExpressionNode::Range(range) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                range.start,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program, machine, facts, range.end, collected,
            );
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_state_argument_facts_from_expression(
                    program,
                    machine,
                    facts,
                    field.value,
                    collected,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
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

fn collect_state_argument_facts_for_call(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    target_symbol: SymbolHandle,
    target_name: Option<&omega_typed_trees::name::Identifier>,
    arguments: &[ExpressionHandle],
    collected: &mut Vec<StateArgumentFacts>,
) {
    let Some(target_state) = program.machine_states(machine).iter().find(|state| {
        (target_symbol.is_valid() && state.symbol == target_symbol)
            || target_name.is_some_and(|target_name| state.name == *target_name)
    }) else {
        return;
    };

    let entry = collected
        .iter_mut()
        .find(|entry| entry.state == target_state.symbol);
    let entry = if let Some(entry) = entry {
        entry
    } else {
        collected.push(StateArgumentFacts {
            state: target_state.symbol,
            parameters: program
                .state_parameters(target_state)
                .iter()
                .map(|parameter| ParameterFacts {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    is_self: parameter.is_self,
                    length: MergedFact::Unseen,
                    integer: MergedFact::Unseen,
                })
                .collect(),
        });
        collected
            .last_mut()
            .expect("state argument facts were just inserted")
    };

    for (parameter, argument) in entry
        .parameters
        .iter_mut()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments.iter().copied())
    {
        parameter
            .length
            .merge(expression_indexable_length(program, facts, argument));
        parameter
            .integer
            .merge(expression_integer_value(program, facts, argument));
    }
}
