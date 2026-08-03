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

use crate::borrow::tracker::BorrowOwnerSegment;

pub(super) fn statement_uses_local_name(
    program: &psi_typed_trees::TypedTrees,
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
    program: &psi_typed_trees::TypedTrees,
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
    program: &psi_typed_trees::TypedTrees,
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
            matches!(transition.guard, psi_typed_trees::statement::TransitionGuardNode::When(expression) if expression_uses(expression))
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

pub(super) fn statement_uses_owner_path(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    owner_symbol: SymbolHandle,
    owner_name: &str,
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    let uses = |expression| {
        expression_uses_owner_path(
            program,
            state_symbol,
            statement_index,
            expression,
            owner_symbol,
            owner_path,
        )
    };
    match statement {
        StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => uses(assignment.target) || uses(assignment.value),
        StatementNode::Call(call) => {
            call.receiver_symbol == owner_symbol
                || crate::lookup::statement_call_receiver_members(program, call)
                    .and_then(|members| members.first())
                    .is_some_and(|member| member.as_str() == owner_name)
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| uses(*argument))
        }
        StatementNode::Expression(expression) => uses(*expression),
        StatementNode::LocalData(local_data) => uses(local_data.initial_value),
        StatementNode::Transition(transition) => {
            matches!(transition.guard, psi_typed_trees::statement::TransitionGuardNode::When(expression) if uses(expression))
                || transition_target_uses_owner_path(
                    program,
                    state_symbol,
                    statement_index,
                    program.statement_table.transition_target(transition.target),
                    owner_symbol,
                    owner_path,
                )
                || (transition.continuation.is_valid()
                    && transition_target_uses_owner_path(
                        program,
                        state_symbol,
                        statement_index,
                        program
                            .statement_table
                            .transition_target(transition.continuation),
                        owner_symbol,
                        owner_path,
                    ))
        }
    }
}

pub(super) fn owner_path_overlaps_place_segments(
    program: &psi_typed_trees::TypedTrees,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[psi_facts::PlaceSegment],
) -> bool {
    owner_path
        .iter()
        .zip(place_segments)
        .all(|(owner, place)| match (owner, place) {
            (
                BorrowOwnerSegment::Field(owner_symbol),
                psi_facts::PlaceSegment::Field {
                    symbol: place_symbol,
                },
            ) => !place_symbol.is_valid() || owner_symbol == place_symbol,
            (
                BorrowOwnerSegment::Case(owner_variant),
                psi_facts::PlaceSegment::Case {
                    variant: place_variant,
                },
            ) => owner_variant == place_variant,
            (
                BorrowOwnerSegment::FixedIndex(owner_index),
                psi_facts::PlaceSegment::FixedIndex { index: place_index },
            ) => owner_index == place_index,
            (
                BorrowOwnerSegment::FixedIndex(owner_index),
                psi_facts::PlaceSegment::Index { expression },
            ) => program
                .expression_table
                .constant_integer_value(*expression)
                .and_then(|value| usize::try_from(value).ok())
                .is_none_or(|place_index| *owner_index == place_index),
            (
                BorrowOwnerSegment::DynamicIndex,
                psi_facts::PlaceSegment::FixedIndex { .. } | psi_facts::PlaceSegment::Index { .. },
            ) => true,
            _ => false,
        })
}

fn expression_uses_owner_path(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    owner_symbol: SymbolHandle,
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    if crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    )
    .is_some_and(|place| {
        matches!(place.root, psi_facts::PlaceRoot::Symbol(root) if root == owner_symbol)
            && owner_path_overlaps_place_segments(program, owner_path, &place.segments)
    }) {
        return true;
    }

    let recurse = |child| {
        expression_uses_owner_path(
            program,
            state_symbol,
            statement_index,
            child,
            owner_symbol,
            owner_path,
        )
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => recurse(atomic.value),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| recurse(*value)),
        ExpressionNode::Binary(binary) => recurse(binary.left) || recurse(binary.right),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && recurse(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| recurse(*argument))
        }
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Indexed(indexed) => recurse(indexed.collection) || recurse(indexed.index),
        ExpressionNode::Range(range) => {
            (range.start.is_valid() && recurse(range.start))
                || (range.end.is_valid() && recurse(range.end))
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| recurse(field.value)),
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Mutable(inner) => recurse(*inner),
        ExpressionNode::Member(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn transition_target_uses_owner_path(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &psi_typed_trees::statement::TransitionTargetNode,
    owner_symbol: SymbolHandle,
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    let uses = |expression| {
        expression_uses_owner_path(
            program,
            state_symbol,
            statement_index,
            expression,
            owner_symbol,
            owner_path,
        )
    };
    match target {
        psi_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .any(|argument| uses(*argument)),
        psi_typed_trees::statement::TransitionTargetNode::Value(expression) => uses(*expression),
        psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        | psi_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

fn transition_target_uses_place_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &psi_typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        psi_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => program
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
        psi_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_place_symbol(
                program,
                state_symbol,
                statement_index,
                *expression,
                symbol,
            )
        }
        psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        | psi_typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}
