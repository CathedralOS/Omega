use crate::context::*;

mod expressions;
mod transitions;

use expressions::{
    expression_uses_local_name, expression_uses_place_symbol, expression_uses_symbol,
};
use transitions::{
    transition_guard_uses_local_name, transition_guard_uses_symbol,
    transition_target_uses_local_name, transition_target_uses_symbol,
};

pub(super) fn statement_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    local_name: &str,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_uses_local_name(program, assignment.target, local_name)
                || expression_uses_local_name(program, assignment.value, local_name)
        }
        StatementNode::Call(call) => {
            program
                .statement_table
                .name_path_members(call.receiver)
                .first()
                .is_some_and(|member| member.as_str() == local_name)
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        StatementNode::Expression(expression) => {
            expression_uses_local_name(program, *expression, local_name)
        }
        StatementNode::LocalData(local_data) => {
            expression_uses_local_name(program, local_data.initial_value, local_name)
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_local_name(program, transition.guard, local_name)
                || transition_target_uses_local_name(
                    program,
                    program.statement_table.transition_target(transition.target),
                    local_name,
                )
                || transition_target_uses_local_name(
                    program,
                    program
                        .statement_table
                        .transition_target(transition.continuation),
                    local_name,
                )
        }
    }
}

pub(super) fn statement_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol(program, assignment.target, symbol)
                || expression_uses_symbol(program, assignment.value, symbol)
        }
        StatementNode::Call(call) => {
            call.receiver_symbol == symbol
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        StatementNode::Expression(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        StatementNode::LocalData(local_data) => {
            expression_uses_symbol(program, local_data.initial_value, symbol)
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_symbol(program, transition.guard, symbol)
                || transition_target_uses_symbol(
                    program,
                    program.statement_table.transition_target(transition.target),
                    symbol,
                )
                || transition_target_uses_symbol(
                    program,
                    program
                        .statement_table
                        .transition_target(transition.continuation),
                    symbol,
                )
        }
    }
}

pub(super) fn statement_uses_place_symbol(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    let expression_uses = |expression| {
        expression_uses_place_symbol(program, state_symbol, statement_index, expression, symbol)
    };
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_uses(assignment.target) || expression_uses(assignment.value)
        }
        StatementNode::Call(call) => {
            call.receiver_symbol == symbol
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses(*argument))
        }
        StatementNode::Expression(expression) => expression_uses(*expression),
        StatementNode::LocalData(local_data) => expression_uses(local_data.initial_value),
        StatementNode::Transition(transition) => {
            matches!(transition.guard, omega_typed_trees::statement::TransitionGuardNode::When(expression) if expression_uses(expression))
                || transition_target_uses_place_symbol(
                    program,
                    state_symbol,
                    statement_index,
                    program.statement_table.transition_target(transition.target),
                    symbol,
                )
                || (transition.continuation.is_valid()
                    && transition_target_uses_place_symbol(
                        program,
                        state_symbol,
                        statement_index,
                        program
                            .statement_table
                            .transition_target(transition.continuation),
                        symbol,
                    ))
        }
    }
}

fn transition_target_uses_place_symbol(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &omega_typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        omega_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .any(|argument| {
                expression_uses_place_symbol(
                    program,
                    state_symbol,
                    statement_index,
                    *argument,
                    symbol,
                )
            }),
        omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                *expression,
                symbol,
            )
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}
