mod aliases;
mod transitions;

use self::aliases::seed_local_alias_facts;
use self::transitions::check_transition_target;
use super::arrays::fixed_array_type_length;
use super::expressions::{expression_indexable_length, expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};
use super::indexes::check_expression;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode};

pub(super) fn check_statement(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.target,
                diagnostics,
            );
            check_expression(
                program,
                machine,
                state,
                facts,
                assignment.value,
                diagnostics,
            );
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
                seed_local_alias_facts(program, facts, assignment.value, name);
            } else if let Some((symbol, name)) = expression_member_name(program, assignment.target)
            {
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_field_integer(symbol, name, next_integer);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
            facts.forget_field_integers();
        }
        StatementNode::Expression(expression) => {
            check_expression(program, machine, state, facts, *expression, diagnostics);
        }
        StatementNode::LocalData(local) => {
            check_expression(
                program,
                machine,
                state,
                facts,
                local.initial_value,
                diagnostics,
            );
            let length = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference));
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
            seed_local_alias_facts(
                program,
                facts,
                local.initial_value,
                Some(local.name.as_str()),
            );
        }
        StatementNode::Transition(transition) => {
            let (target_facts, continuation_facts) = match transition.guard {
                TransitionGuardNode::When(guard) => {
                    check_expression(program, machine, state, facts, guard, diagnostics);
                    let mut guarded_facts = facts.clone();
                    seed_guard_facts(program, &mut guarded_facts, guard);
                    let mut negated_facts = facts.clone();
                    seed_negated_guard_facts(program, &mut negated_facts, guard);
                    (guarded_facts, negated_facts)
                }
                TransitionGuardNode::Always => (facts.clone(), facts.clone()),
            };
            check_transition_target(
                program,
                machine,
                state,
                &target_facts,
                transition.target,
                diagnostics,
            );
            check_transition_target(
                program,
                machine,
                state,
                &continuation_facts,
                transition.continuation,
                diagnostics,
            );
        }
    }
}

fn expression_member_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(omega_core::symbols::SymbolHandle, Option<&str>)> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    Some((member.member_symbol, Some(member.member.as_str())))
}
