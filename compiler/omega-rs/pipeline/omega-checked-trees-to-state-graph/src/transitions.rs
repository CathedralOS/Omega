mod targets;

use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::statement::TableCall;
use psi_diagnostics::Diagnostic;

use crate::segments::{
    SegmentTransition, StateSegment, copy_statement_expression_span,
    table_transition_guard_expression,
};
use crate::transitions::targets::{next_segment_target, plan_call_target, plan_transition_target};
use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateKey, TransitionEdge, TransitionExpressionRefs,
};

pub(super) fn plan_transition(
    source_key: StateKey,
    segments: &[StateSegment],
    transition: &SegmentTransition,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> Result<TransitionEdge, Diagnostic> {
    match transition {
        SegmentTransition::Tree {
            statement_index,
            table,
        } => {
            let target_arguments =
                table_transition_target_arguments(table.target, program, state_graph);
            let target_value = table_transition_target_value(table.target, program, state_graph);
            let continuation_arguments = table
                .continuation
                .is_valid()
                .then(|| {
                    table_transition_target_arguments(table.continuation, program, state_graph)
                })
                .unwrap_or_default();
            let continuation_value = table
                .continuation
                .is_valid()
                .then(|| table_transition_target_value(table.continuation, program, state_graph))
                .unwrap_or_else(ExpressionHandle::invalid);
            let guard_expression = table_transition_guard_expression(*table);
            let guard_expression = guard_expression
                .is_valid()
                .then(|| {
                    state_graph
                        .expressions
                        .copy_from(&program.expression_table, guard_expression)
                })
                .unwrap_or_else(ExpressionHandle::invalid);

            Ok(TransitionEdge {
                statement_index: *statement_index,
                target: plan_transition_target(source_key, segments, table.target, program)?,
                continuation: if table.continuation.is_valid() {
                    plan_transition_target(source_key, segments, table.continuation, program)?
                } else {
                    PlannedTransitionTarget::None
                },
                expressions: TransitionExpressionRefs {
                    target_arguments,
                    target_value,
                    continuation_arguments,
                    continuation_value,
                    guard: guard_expression,
                },
            })
        }
        SegmentTransition::ReturnExpression {
            statement_index,
            expression,
        } => Ok(TransitionEdge {
            statement_index: *statement_index,
            target: PlannedTransitionTarget::Terminal,
            continuation: PlannedTransitionTarget::None,
            expressions: TransitionExpressionRefs {
                target_arguments: psi_arena::HandleSpan::empty(),
                target_value: state_graph
                    .expressions
                    .copy_from(&program.expression_table, *expression),
                continuation_arguments: psi_arena::HandleSpan::empty(),
                continuation_value: ExpressionHandle::invalid(),
                guard: ExpressionHandle::invalid(),
            },
        }),
        SegmentTransition::BranchCall {
            statement_index,
            has_continuation_segment,
        } => {
            let table = branch_call_statement(program, source_key, *statement_index)?;
            Ok(TransitionEdge {
                statement_index: *statement_index,
                target: plan_call_target(source_key, segments, table, program)?,
                continuation: if *has_continuation_segment {
                    next_segment_target(source_key, segments)?
                } else {
                    PlannedTransitionTarget::None
                },
                expressions: TransitionExpressionRefs {
                    target_arguments: copy_statement_expression_span(
                        state_graph,
                        &program.expression_table,
                        &program.statement_table,
                        table.arguments,
                    ),
                    target_value: ExpressionHandle::invalid(),
                    continuation_arguments: psi_arena::HandleSpan::empty(),
                    continuation_value: ExpressionHandle::invalid(),
                    guard: ExpressionHandle::invalid(),
                },
            })
        }
    }
}

fn branch_call_statement(
    program: &CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
) -> Result<&TableCall, Diagnostic> {
    let state = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == source_key.state)
        })
        .ok_or_else(|| Diagnostic::error("internal branch-call source state was not indexed"))?;

    match program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    {
        Some(psi_checked_trees::statement::StatementNode::Call(call)) => Ok(call),
        _ => Err(Diagnostic::error(
            "internal branch-call segment did not reference a call statement",
        )),
    }
}

fn table_transition_target_arguments(
    target: psi_checked_trees::statement::TransitionTargetHandle,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> psi_arena::HandleSpan<psi_checked_trees::expression::ExpressionHandle> {
    if !target.is_valid() {
        return psi_arena::HandleSpan::empty();
    }

    match program.statement_table.transition_target(target) {
        psi_checked_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            copy_statement_expression_span(
                state_graph,
                &program.expression_table,
                &program.statement_table,
                *arguments,
            )
        }
        psi_checked_trees::statement::TransitionTargetNode::SelfTarget
        | psi_checked_trees::statement::TransitionTargetNode::Terminal
        | psi_checked_trees::statement::TransitionTargetNode::Value(_) => {
            psi_arena::HandleSpan::empty()
        }
    }
}

fn table_transition_target_value(
    target: psi_checked_trees::statement::TransitionTargetHandle,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> psi_checked_trees::expression::ExpressionHandle {
    if !target.is_valid() {
        return psi_checked_trees::expression::ExpressionHandle::invalid();
    }

    match program.statement_table.transition_target(target) {
        psi_checked_trees::statement::TransitionTargetNode::Value(expression) => state_graph
            .expressions
            .copy_from(&program.expression_table, *expression),
        _ => psi_checked_trees::expression::ExpressionHandle::invalid(),
    }
}
