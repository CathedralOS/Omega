use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

mod expressions;
mod transitions;

use expressions::expression_uses_local_name;
use transitions::{transition_guard_uses_local_name, transition_target_uses_local_name};
pub(super) use validation::statement_uses_symbol;

use crate::borrow::owner_segment_matches_place_segment;
use crate::borrow::tracker::BorrowOwnerSegment;
use crate::places::place_at;

pub(super) fn statement_uses_local_name(
    program: &dyn ::validation::CheckingInput,
    statement: &StatementNode,
    local_name: &str,
) -> bool {
    match statement {
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
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

pub(super) fn statement_uses_owner_path(
    program: &dyn ::validation::CheckingInput,
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
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => uses(assignment.target) || uses(assignment.value),
        StatementNode::Call(call) => {
            call.receiver_symbol == owner_symbol
                || validation::statement_call_receiver_members(program, call)
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
            matches!(transition.guard, typed_trees::statement::TransitionGuardNode::When(expression) if uses(expression))
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

pub(crate) fn owner_path_overlaps_place_segments(
    program: &dyn ::validation::CheckingInput,
    owner_path: &[BorrowOwnerSegment],
    place_segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.iter().zip(place_segments).all(|(owner, place)| {
        owner_segment_matches_place_segment(program, owner, place).unwrap_or_else(|| {
            matches!(
                (owner, place),
                (
                    BorrowOwnerSegment::DynamicIndex,
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
                )
            )
        })
    })
}

fn expression_uses_owner_path(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    owner_symbol: SymbolHandle,
    owner_path: &[BorrowOwnerSegment],
) -> bool {
    let edges = typed_trees::SubtreeEdges {
        pattern_dependencies: true,
        ..typed_trees::SubtreeEdges::SAVED_SOURCES
    };
    program.subtree_any(expression, edges, |handle, node| {
        if place_at(program, state_symbol, statement_index, handle).is_some_and(|place| {
            matches!(place.root, facts::PlaceRoot::Symbol(root) if root == owner_symbol)
                && owner_path_overlaps_place_segments(program, owner_path, &place.segments)
        }) {
            typed_trees::SubtreeVisit::Found
        } else if matches!(node, ExpressionNode::Member(_)) {
            typed_trees::SubtreeVisit::Prune
        } else {
            typed_trees::SubtreeVisit::Descend
        }
    })
}

fn transition_target_uses_owner_path(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target: &typed_trees::statement::TransitionTargetNode,
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
        typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => program
            .statement_table
            .expression_handles(*arguments)
            .iter()
            .any(|argument| uses(*argument)),
        typed_trees::statement::TransitionTargetNode::Value(expression) => uses(*expression),
        typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}
