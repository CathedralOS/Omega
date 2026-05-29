use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::StateArgumentFacts;
use super::calls::collect_state_argument_facts_for_call;
use super::expressions::collect_state_argument_facts_from_expression;
use crate::checks::ranges::arrays::fixed_array_type_length;
use crate::checks::ranges::expressions::{
    expression_indexable_length, expression_integer_value, expression_name,
};
use crate::checks::ranges::facts::RangeFacts;

pub(super) fn collect_state_argument_facts_from_statement(
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
