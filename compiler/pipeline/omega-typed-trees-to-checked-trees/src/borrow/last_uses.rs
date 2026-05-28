use crate::context::*;
use crate::lookup::first_valid_name_path_symbol;

use super::StateLoanTracker;

pub(super) fn update_state_loan_last_uses(
    program: &omega_typed_trees::TypedTrees,
    statements: omega_core::arena::HandleSpan<StatementNode>,
    borrow_calls: &[BorrowCallFact],
    argument_accesses: &omega_core::arena::Arena<BorrowArgumentAccessFact>,
    loan_trackers: &[StateLoanTracker],
    loans: &mut omega_core::arena::Arena<omega_checked_trees::BorrowLoanFact>,
) {
    if loan_trackers.is_empty() {
        return;
    }

    for borrow_call in borrow_calls {
        for access in argument_accesses.span_or_empty(borrow_call.accesses) {
            for tracker in loan_trackers {
                if tracker.owner_symbol == access.root_symbol {
                    loans.get_mut(tracker.handle).last_use_statement_index =
                        borrow_call.statement_index;
                }
            }
        }
    }

    for (statement_index, statement) in program
        .statement_table
        .statements(statements)
        .iter()
        .enumerate()
    {
        for tracker in loan_trackers {
            if statement_uses_local_name(program, statement, tracker.owner_name.as_str())
                || statement_uses_symbol(program, statement, tracker.owner_symbol)
            {
                loans.get_mut(tracker.handle).last_use_statement_index = statement_index;
            }
        }
    }
}

fn statement_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    local_name: &str,
) -> bool {
    match statement {
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

fn statement_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    match statement {
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

fn transition_guard_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    guard: omega_typed_trees::statement::TransitionGuardNode,
    symbol: SymbolHandle,
) -> bool {
    match guard {
        omega_typed_trees::statement::TransitionGuardNode::Always => false,
        omega_typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_symbol(program, expression, symbol)
        }
    }
}

fn transition_guard_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    guard: omega_typed_trees::statement::TransitionGuardNode,
    local_name: &str,
) -> bool {
    match guard {
        omega_typed_trees::statement::TransitionGuardNode::Always => false,
        omega_typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_local_name(program, expression, local_name)
        }
    }
}

fn transition_target_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    target: &omega_typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        omega_typed_trees::statement::TransitionTargetNode::Named { path, arguments } => {
            path.head_symbol == symbol
                || path.symbol == symbol
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

fn transition_target_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    target: &omega_typed_trees::statement::TransitionTargetNode,
    local_name: &str,
) -> bool {
    match target {
        omega_typed_trees::statement::TransitionTargetNode::Named { path, arguments } => {
            program
                .statement_table
                .name_path_members(path.members)
                .first()
                .is_some_and(|member| member.as_str() == local_name)
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        omega_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_local_name(program, *expression, local_name)
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

fn expression_uses_symbol(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_symbol(program, *value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_symbol(program, binary.left, symbol)
                || expression_uses_symbol(program, binary.right, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_uses_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_symbol(program, cast.value, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol(program, indexed.collection, symbol)
                || expression_uses_symbol(program, indexed.index, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_symbol(program, range.start, symbol))
                || (range.end.is_valid() && expression_uses_symbol(program, range.end, symbol))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_symbol(program, member.receiver, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_symbol(program, *inner_expression, symbol)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => {
            first_valid_name_path_symbol(path, &program.expression_table)
                .is_some_and(|path_symbol| path_symbol == symbol)
                || path.symbol == symbol
        }
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_symbol(program, field.value, symbol)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}

fn expression_uses_local_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    local_name: &str,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_local_name(program, *value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_local_name(program, binary.left, local_name)
                || expression_uses_local_name(program, binary.right, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid()
                && expression_uses_local_name(program, call.receiver, local_name))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_local_name(program, cast.value, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_local_name(program, indexed.collection, local_name)
                || expression_uses_local_name(program, indexed.index, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_local_name(program, range.start, local_name))
                || (range.end.is_valid()
                    && expression_uses_local_name(program, range.end, local_name))
        }
        omega_typed_trees::expression::ExpressionNode::Member(member) => {
            expression_uses_local_name(program, member.receiver, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Mutable(inner_expression) => {
            expression_uses_local_name(program, *inner_expression, local_name)
        }
        omega_typed_trees::expression::ExpressionNode::Name(path) => program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|member| member.as_str() == local_name),
        omega_typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_local_name(program, field.value, local_name)),
        omega_typed_trees::expression::ExpressionNode::Boolean(_)
        | omega_typed_trees::expression::ExpressionNode::Float(_)
        | omega_typed_trees::expression::ExpressionNode::Integer(_)
        | omega_typed_trees::expression::ExpressionNode::String(_) => false,
    }
}
