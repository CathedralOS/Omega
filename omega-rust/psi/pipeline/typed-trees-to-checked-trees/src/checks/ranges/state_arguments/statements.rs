use typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::calls::collect_state_argument_facts_for_call;
use super::expressions::collect_state_argument_facts_from_expression;
use super::{StateArgumentContext, StateArgumentFacts};
use crate::checks::ranges::arrays::fixed_array_type_length;
use crate::checks::ranges::expressions::{
    expression_indexable_length, expression_integer_value, expression_name,
};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::guards;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn collect_state_argument_facts_from_statement(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let program = context.program;
    let machine = context.machine;
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            collect_state_argument_facts_from_expression(
                context,
                facts,
                assignment.target,
                collected,
            );
            collect_state_argument_facts_from_expression(
                context,
                facts,
                assignment.value,
                collected,
            );
            // RHS effects and values are evaluated before replacing the target.
            let next_length = expression_indexable_length(program, facts, assignment.value);
            let next_integer = expression_integer_value(program, facts, assignment.value);
            facts.invalidate_assignment_bounds(
                &program.expression_table.display_name(assignment.target),
            );
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                facts.assign_local(symbol, name, next_length, next_integer);
                seed_boolean_guard_local(context, facts, symbol, name, assignment.value);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_state_argument_facts_from_expression(context, facts, *argument, collected);
            }
            collect_state_argument_facts_for_call(
                program,
                machine,
                facts,
                call.target_symbol,
                program.statement_table.expression_handles(call.arguments),
                collected,
            );
            let paths = context
                .call_frames
                .and_then(|frames| frames.may_write_frame(machine, call).into_complete_paths());
            facts.invalidate_call_writes(program, context.state, paths.as_deref());
            // R4 witness mint in the COLLECTION pass too: boundary ensures
            // bound the &mut argument places, so a later transition can
            // transport the fact into its target's params.
            crate::checks::ranges::statements::seed_boundary_call_ensures_facts(
                program, machine, call, facts,
            );
        }
        StatementNode::Expression(expression) => {
            collect_state_argument_facts_from_expression(context, facts, *expression, collected);
        }
        StatementNode::LocalData(local) => {
            collect_state_argument_facts_from_expression(
                context,
                facts,
                local.initial_value,
                collected,
            );
            let length = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference));
            let integer = expression_integer_value(program, facts, local.initial_value);
            facts.define_local(local.symbol, local.name.to_string(), length, integer);
            seed_boolean_guard_local(
                context,
                facts,
                local.symbol,
                Some(local.name.as_str()),
                local.initial_value,
            );
        }
        StatementNode::Transition(transition) => {
            // A guard established before a recursive / cyclic transition refines
            // the facts that flow into the callee's arguments. The guard's
            // positive form constrains the branch that is actually taken
            // (`transition.target`), so narrow a working copy of the facts with
            // it before deriving the target's argument facts.
            let guarded_facts = match transition.guard {
                typed_trees::statement::TransitionGuardNode::When(guard) if guard.is_valid() => {
                    collect_state_argument_facts_from_expression(context, facts, guard, collected);
                    let mut narrowed = facts.clone();
                    if context.call_frames.is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, guard)
                            .into_complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }) {
                        guards::seed_guard_facts(
                            program,
                            machine,
                            context.state,
                            &mut narrowed,
                            guard,
                        );
                    }
                    Some(narrowed)
                }
                _ => None,
            };
            let mut target_facts = guarded_facts.unwrap_or_else(|| facts.clone());

            collect_state_argument_facts_from_target(
                context,
                &mut target_facts,
                transition.target,
                collected,
            );
            // The continuation branch is taken when the guard does not hold, so
            // it is analysed with the unrefined facts.
            let mut continuation_facts = facts.clone();
            collect_state_argument_facts_from_target(
                context,
                &mut continuation_facts,
                transition.continuation,
                collected,
            );
        }
    }
}

fn collect_state_argument_facts_from_target(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    target: typed_trees::statement::TransitionTargetHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    let program = context.program;
    let machine = context.machine;
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_state_argument_facts_from_expression(context, facts, *argument, collected);
            }
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
                program.statement_table.expression_handles(*arguments),
                collected,
            );
        }
        TransitionTargetNode::Value(value) => {
            collect_state_argument_facts_from_expression(context, facts, *value, collected);
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn seed_boolean_guard_local(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    symbol: symbols::SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    let program = context.program;
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) && context.call_frames.is_some_and(|frames| {
        frames
            .expression_write_frame(context.machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
    }) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}
