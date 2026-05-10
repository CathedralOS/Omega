use omega_core::diagnostics::Diagnostic;
use omega_typed_program::Program;
use omega_typed_program::expression::display_name_path;
use omega_typed_program::statement::TransitionTarget;

use crate::segments::{
    SegmentTransition, copy_statement_expression_span, table_transition_guard_expression,
};
use omega_control_flow::{
    ControlFlowPlan, PlannedTransitionTarget, StateKey, TransitionExpressionRefs, TransitionFlow,
};

pub(super) fn plan_transition(
    state_indexes: &[(StateKey, usize)],
    transition: &SegmentTransition<'_>,
    program: &Program,
    control_flow: &mut ControlFlowPlan,
) -> Result<TransitionFlow, Diagnostic> {
    let tree = transition.tree;
    let target_arguments =
        table_transition_target_arguments(transition.table.target, program, control_flow);
    let continuation_arguments = transition
        .table
        .continuation
        .is_valid()
        .then(|| {
            table_transition_target_arguments(transition.table.continuation, program, control_flow)
        })
        .unwrap_or_default();
    let guard = table_transition_guard_expression(transition.table).map(|guard| {
        control_flow
            .expressions
            .copy_from(&program.expression_table, guard)
    });

    Ok(TransitionFlow {
        target: plan_transition_target(state_indexes, &tree.target)?,
        continuation: tree
            .continuation
            .as_ref()
            .map(|target| plan_transition_target(state_indexes, target))
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
    target: omega_typed_program::statement::TransitionTargetHandle,
    program: &Program,
    control_flow: &mut ControlFlowPlan,
) -> omega_core::arena::HandleSpan<omega_typed_program::expression::ExpressionHandle> {
    if !target.is_valid() {
        return omega_core::arena::HandleSpan::empty();
    }

    match program.statement_table.transition_target(target) {
        omega_typed_program::statement::TransitionTargetNode::Named { arguments, .. } => {
            copy_statement_expression_span(
                control_flow,
                &program.expression_table,
                &program.statement_table,
                *arguments,
            )
        }
        omega_typed_program::statement::TransitionTargetNode::SelfTarget
        | omega_typed_program::statement::TransitionTargetNode::Terminal => {
            omega_core::arena::HandleSpan::empty()
        }
    }
}

fn plan_transition_target(
    state_indexes: &[(StateKey, usize)],
    target: &TransitionTarget,
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
                    .find(|(key, _)| key.state == symbol && key.segment_index == 0)
            });
            let (key, index) = target.flatten().ok_or_else(|| {
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
    }
}
