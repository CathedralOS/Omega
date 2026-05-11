use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::Program;
use omega_typed_trees::expression::display_name_path;
use omega_typed_trees::statement::TransitionTarget;

use crate::segments::{
    SegmentTransition, copy_statement_expression_span, table_transition_guard_expression,
};
use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateKey, TransitionEdge, TransitionExpressionRefs,
};

pub(super) fn plan_transition(
    state_indexes: &[(StateKey, usize, omega_typed_trees::name::ProgramName)],
    transition: &SegmentTransition<'_>,
    program: &Program,
    state_graph: &mut StateGraph,
) -> Result<TransitionEdge, Diagnostic> {
    let tree = transition.tree;
    let target_arguments =
        table_transition_target_arguments(transition.table.target, program, state_graph);
    let continuation_arguments = transition
        .table
        .continuation
        .is_valid()
        .then(|| {
            table_transition_target_arguments(transition.table.continuation, program, state_graph)
        })
        .unwrap_or_default();
    let guard = table_transition_guard_expression(transition.table).map(|guard| {
        state_graph
            .expressions
            .copy_from(&program.expression_table, guard)
    });

    Ok(TransitionEdge {
        target: plan_transition_target(state_indexes, &tree.target, state_graph)?,
        continuation: tree
            .continuation
            .as_ref()
            .map(|target| plan_transition_target(state_indexes, target, state_graph))
            .transpose()?,
        guard: tree.guard.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments,
            continuation_arguments,
            guard,
        },
    })
}

fn table_transition_target_arguments(
    target: omega_typed_trees::statement::TransitionTargetHandle,
    program: &Program,
    state_graph: &mut StateGraph,
) -> omega_core::arena::HandleSpan<omega_typed_trees::expression::ExpressionHandle> {
    if !target.is_valid() {
        return omega_core::arena::HandleSpan::empty();
    }

    match program.statement_table.transition_target(target) {
        omega_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            copy_statement_expression_span(
                state_graph,
                &program.expression_table,
                &program.statement_table,
                *arguments,
            )
        }
        omega_typed_trees::statement::TransitionTargetNode::SelfTarget
        | omega_typed_trees::statement::TransitionTargetNode::Terminal
        | omega_typed_trees::statement::TransitionTargetNode::Value(_) => {
            omega_core::arena::HandleSpan::empty()
        }
    }
}

fn plan_transition_target(
    state_indexes: &[(StateKey, usize, omega_typed_trees::name::ProgramName)],
    target: &TransitionTarget,
    _state_graph: &StateGraph,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    match target {
        TransitionTarget::Named {
            path, arguments, ..
        } if path.len() == 1 || path.len() == 2 && path[0] == "self" => {
            let name = path.last().expect("named transition has a state").clone();
            let symbol = path.symbol();
            let target = symbol.is_valid().then(|| {
                state_indexes
                    .iter()
                    .find(|(key, _, _)| key.state == symbol && key.segment_index == 0)
            }).flatten().or_else(|| {
                state_indexes.iter().find(|(key, _, state_name)| {
                    key.segment_index == 0 && *state_name == name
                })
            });
            let (key, index, _) = target.ok_or_else(|| {
                Diagnostic::error(format!("unknown state transition target `{name}`"))
            })?;

            Ok(PlannedTransitionTarget::State {
                index: *index,
                key: *key,
                name,
            })
        }
        TransitionTarget::Named {
            path, arguments: _, ..
        } if path.len() == 2 => Ok(PlannedTransitionTarget::Nested {
            receiver_symbol: path.head_symbol(),
            state_symbol: path.symbol(),
            receiver: path[0].clone(),
            state: path[1].clone(),
        }),
        TransitionTarget::Named { path, .. } => Err(Diagnostic::error(format!(
            "unsupported transition target `{}`",
            display_name_path(path, ".")
        ))),
        TransitionTarget::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTarget::Terminal => Ok(PlannedTransitionTarget::Terminal),
        TransitionTarget::Value(_) => Err(Diagnostic::error(
            "value transition targets are not lowered to the state graph yet",
        )),
    }
}
