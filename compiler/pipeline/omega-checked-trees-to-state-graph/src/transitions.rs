use omega_checked_trees::Program;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::statement::{TableCall, TransitionTargetHandle, TransitionTargetNode};
use omega_core::diagnostics::Diagnostic;

use crate::segments::{
    SegmentTransition, StateSegment, copy_statement_expression_span,
    table_transition_guard_expression,
};
use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateKey, TransitionEdge, TransitionExpressionRefs,
};

pub(super) fn plan_transition(
    source_key: StateKey,
    segments: &[StateSegment],
    transition: &SegmentTransition,
    program: &Program,
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
                target_arguments: omega_core::arena::HandleSpan::empty(),
                target_value: state_graph
                    .expressions
                    .copy_from(&program.expression_table, *expression),
                continuation_arguments: omega_core::arena::HandleSpan::empty(),
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
                    continuation_arguments: omega_core::arena::HandleSpan::empty(),
                    continuation_value: ExpressionHandle::invalid(),
                    guard: ExpressionHandle::invalid(),
                },
            })
        }
    }
}

fn branch_call_statement(
    program: &Program,
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
        Some(omega_checked_trees::statement::StatementNode::Call(call)) => Ok(call),
        _ => Err(Diagnostic::error(
            "internal branch-call segment did not reference a call statement",
        )),
    }
}

fn table_transition_target_arguments(
    target: omega_checked_trees::statement::TransitionTargetHandle,
    program: &Program,
    state_graph: &mut StateGraph,
) -> omega_core::arena::HandleSpan<omega_checked_trees::expression::ExpressionHandle> {
    if !target.is_valid() {
        return omega_core::arena::HandleSpan::empty();
    }

    match program.statement_table.transition_target(target) {
        omega_checked_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            copy_statement_expression_span(
                state_graph,
                &program.expression_table,
                &program.statement_table,
                *arguments,
            )
        }
        omega_checked_trees::statement::TransitionTargetNode::SelfTarget
        | omega_checked_trees::statement::TransitionTargetNode::Terminal
        | omega_checked_trees::statement::TransitionTargetNode::Value(_) => {
            omega_core::arena::HandleSpan::empty()
        }
    }
}

fn table_transition_target_value(
    target: omega_checked_trees::statement::TransitionTargetHandle,
    program: &Program,
    state_graph: &mut StateGraph,
) -> omega_checked_trees::expression::ExpressionHandle {
    if !target.is_valid() {
        return omega_checked_trees::expression::ExpressionHandle::invalid();
    }

    match program.statement_table.transition_target(target) {
        omega_checked_trees::statement::TransitionTargetNode::Value(expression) => state_graph
            .expressions
            .copy_from(&program.expression_table, *expression),
        _ => omega_checked_trees::expression::ExpressionHandle::invalid(),
    }
}

fn plan_transition_target(
    source_key: StateKey,
    segments: &[StateSegment],
    target: TransitionTargetHandle,
    program: &Program,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    if !target.is_valid() {
        return Ok(PlannedTransitionTarget::Terminal);
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, arguments: _ }
            if is_local_transition_path(
                source_key,
                program.statement_table.name_path_members(path.members),
                path.head_symbol,
            ) =>
        {
            let members = program.statement_table.name_path_members(path.members);
            let name = members
                .last()
                .expect("named transition has a state")
                .clone();
            let symbol = path.symbol;
            let target = symbol
                .is_valid()
                .then(|| find_initial_segment_by_symbol(segments, symbol))
                .flatten()
                .or_else(|| find_initial_segment_by_name(segments, &name));
            let Some(target) = target else {
                if members.len() == 2 {
                    return Ok(PlannedTransitionTarget::Nested {
                        receiver_symbol: path.head_symbol,
                        state_symbol: path.symbol,
                        receiver: members[0].clone(),
                        state: members[1].clone(),
                    });
                }

                return Err(Diagnostic::error(format!(
                    "unknown state transition target `{name}`"
                )));
            };

            Ok(PlannedTransitionTarget::State {
                index: target.0,
                key: target.1.key,
                name,
            })
        }
        TransitionTargetNode::Named { path, arguments: _ } => {
            let members = program.statement_table.name_path_members(path.members);
            if members.len() == 2 {
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: path.head_symbol,
                    state_symbol: path.symbol,
                    receiver: members[0].clone(),
                    state: members[1].clone(),
                });
            }

            Err(Diagnostic::error(format!(
                "unsupported transition target `{}`",
                display_transition_path(members)
            )))
        }
        TransitionTargetNode::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
            Ok(PlannedTransitionTarget::Terminal)
        }
    }
}

fn plan_call_target(
    source_key: StateKey,
    segments: &[StateSegment],
    call: &TableCall,
    program: &Program,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let receiver = program.statement_table.name_path_members(call.receiver);

    if receiver.is_empty() || call.receiver_symbol == source_key.machine {
        let name = call.target.clone();
        let symbol = call.target_symbol;
        let target = symbol
            .is_valid()
            .then(|| find_initial_segment_by_symbol(segments, symbol))
            .flatten()
            .or_else(|| find_initial_segment_by_name(segments, &name));
        let Some(target) = target else {
            if receiver.len() == 1 && receiver[0].as_str() == "self" {
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: call.receiver_symbol,
                    state_symbol: call.target_symbol,
                    receiver: receiver[0].clone(),
                    state: call.target.clone(),
                });
            }

            return Err(Diagnostic::error(format!(
                "unknown state call target `{name}`"
            )));
        };

        return Ok(PlannedTransitionTarget::State {
            index: target.0,
            key: target.1.key,
            name,
        });
    }

    let receiver = receiver.last().cloned().unwrap_or_default();
    Ok(PlannedTransitionTarget::Nested {
        receiver_symbol: call.receiver_symbol,
        state_symbol: call.target_symbol,
        receiver,
        state: call.target.clone(),
    })
}

fn is_local_transition_path(
    source_key: StateKey,
    path: &[omega_checked_trees::name::ProgramName],
    head_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    path.len() == 1 || path.len() == 2 && head_symbol == source_key.machine
}

fn display_transition_path(path: &[omega_checked_trees::name::ProgramName]) -> String {
    let mut display = String::new();

    for member in path {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(member.as_str());
    }

    display
}

fn next_segment_target(
    source_key: StateKey,
    segments: &[StateSegment],
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let next_key = StateKey {
        segment_index: source_key.segment_index + 1,
        ..source_key
    };
    let target = segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key == next_key)
        .ok_or_else(|| {
            Diagnostic::error("internal state-call continuation segment was not indexed")
        })?;

    Ok(PlannedTransitionTarget::State {
        index: target.0,
        key: target.1.key,
        name: target.1.name.clone(),
    })
}

fn find_initial_segment_by_symbol(
    segments: &[StateSegment],
    symbol: omega_core::symbols::SymbolHandle,
) -> Option<(usize, &StateSegment)> {
    segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key.state == symbol && segment.key.segment_index == 0)
}

fn find_initial_segment_by_name<'segments>(
    segments: &'segments [StateSegment],
    name: &omega_checked_trees::name::ProgramName,
) -> Option<(usize, &'segments StateSegment)> {
    segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key.segment_index == 0 && segment.name == *name)
}
