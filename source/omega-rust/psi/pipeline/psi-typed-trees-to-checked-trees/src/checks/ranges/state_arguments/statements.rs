use psi_typed_trees::machine::Machine;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};

use super::StateArgumentFacts;
use super::calls::collect_state_argument_facts_for_call;
use super::expressions::collect_state_argument_facts_from_expression;
use crate::checks::ranges::arrays::fixed_array_type_length;
use crate::checks::ranges::expressions::{
    expression_indexable_length, expression_integer_value, expression_name,
};
use crate::checks::ranges::facts::RangeFacts;
use crate::checks::ranges::guards;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn collect_state_argument_facts_from_statement(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    collected: &mut Vec<StateArgumentFacts>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            // A rebind stales any proven upper bound for the target (mirror
            // of the main walk's Assignment arm).
            facts.forget_index_upper_bound(
                &program.expression_table.display_name(assignment.target),
            );
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
                seed_boolean_guard_local(program, facts, symbol, name, assignment.value);
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
            // R4 witness mint in the COLLECTION pass too: boundary ensures
            // bound the &mut argument places, so a later transition can
            // transport the fact into its target's params.
            crate::checks::ranges::statements::seed_boundary_call_ensures_facts(
                program, machine, call, facts,
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
            seed_boolean_guard_local(
                program,
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
                psi_typed_trees::statement::TransitionGuardNode::When(guard)
                    if guard.is_valid() =>
                {
                    let mut narrowed = facts.clone();
                    guards::seed_guard_facts(program, &mut narrowed, guard);
                    Some(narrowed)
                }
                _ => None,
            };
            let target_facts = guarded_facts.as_ref().unwrap_or(facts);

            collect_state_argument_facts_from_target(
                program,
                machine,
                target_facts,
                transition.target,
                collected,
            );
            // The continuation branch is taken when the guard does not hold, so
            // it is analysed with the unrefined facts.
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
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
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
        TransitionTargetNode::Value(value) => {
            collect_state_argument_facts_from_expression(
                program, machine, facts, *value, collected,
            );
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn seed_boolean_guard_local(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    symbol: psi_symbols::SymbolHandle,
    name: Option<&str>,
    expression: ExpressionHandle,
) {
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(_)
    ) {
        facts.define_boolean_guard_local(symbol, name.unwrap_or_default().to_owned(), expression);
    }
}
