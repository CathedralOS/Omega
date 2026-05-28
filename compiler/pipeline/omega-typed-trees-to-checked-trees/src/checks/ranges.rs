mod diagnostics;
mod expressions;
mod facts;
mod guards;
mod indexes;
mod proofs;
mod requirements;
mod state_arguments;

use expressions::{expression_indexable_length, expression_integer_value, expression_name};
use facts::{RangeFacts, fixed_array_field_lengths, fixed_array_type_length};
use guards::{seed_guard_facts, seed_negated_guard_facts};
use indexes::check_expression;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use requirements::seed_machine_requires;
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};

pub(crate) fn check_indexed_accesses(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        let state_argument_facts = collect_state_argument_facts(program, &field_lengths, machine);
        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(&field_lengths);
            seed_field_integer_facts(program, &mut facts, machine);
            seed_machine_requires(program, &mut facts, machine);
            seed_state_argument_facts(&mut facts, state, &state_argument_facts);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(
                    program,
                    machine,
                    state,
                    &mut facts,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_statement(
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

fn seed_field_integer_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    machine: &Machine,
) {
    for data in program.data_definitions() {
        for member in program.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            if !field.initial_value.is_valid() {
                continue;
            }
            let Some(integer) = expression_integer_value(program, facts, field.initial_value)
            else {
                continue;
            };
            facts.define_field_integer(field.symbol, field.name.to_string(), integer);
        }
    }

    for owned in program.machine_owned_data(machine) {
        if !owned.initial_value.is_valid() {
            continue;
        }
        let Some(integer) = expression_integer_value(program, facts, owned.initial_value) else {
            continue;
        };
        facts.define_field_integer(owned.symbol, owned.name.to_string(), integer);
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

fn seed_local_alias_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    value: ExpressionHandle,
    local_name: Option<&str>,
) {
    if !value.is_valid() {
        return;
    }
    let Some(local_name) = local_name else {
        return;
    };
    let Some(source) = alias_source_label(program, value) else {
        return;
    };

    facts.alias_collection(&source, local_name);
    facts.alias_index(&source, local_name);
}

fn alias_source_label(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            Some(program.expression_table.display_name(call.receiver))
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            Some(program.expression_table.display_name(expression))
        }
        _ => None,
    }
}

fn check_transition_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => {
            check_expression(program, machine, state, facts, *value, diagnostics)
        }
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}
