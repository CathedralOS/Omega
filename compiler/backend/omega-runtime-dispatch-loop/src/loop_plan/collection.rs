use super::context::RuntimeDispatchLoopContext;
use super::guards::{dispatch_guard_comparison, dispatch_guard_expression};
use super::model::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_control_flow::StateKey;
use omega_state_dispatch::DispatchState;
use omega_state_graph::RuntimeTransitionTarget;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchLoopCase {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub operation_count: usize,
    pub edges: Vec<RuntimeDispatchLoopEdge>,
}

pub(super) fn build_runtime_dispatch_loop_case(
    context: &RuntimeDispatchLoopContext,
    state: &DispatchState,
) -> CollectedRuntimeDispatchLoopCase {
    let Some(case_edges) = context.state_dispatch.edges.span(state.edges) else {
        return CollectedRuntimeDispatchLoopCase {
            key: state.key,
            dispatch_index: state.dispatch_index,
            operation_count: runtime_body_operation_count(context, state.dispatch_index),
            edges: Vec::new(),
        };
    };
    let edges = case_edges
        .iter()
        .enumerate()
        .map(|(order, edge)| {
            let guard_comparison = dispatch_guard_comparison(context, state.dispatch_index, order);
            let guard_expression = dispatch_guard_expression(context, state.dispatch_index, order);
            RuntimeDispatchLoopEdge {
                order,
                statement_index: edge.statement_index,
                target: edge.target.clone(),
                target_dispatch_index: edge.target_dispatch_index,
                target_arguments: edge.expressions.target_arguments,
                continuation: edge.continuation.clone(),
                continuation_dispatch_index: edge.continuation_dispatch_index,
                continuation_arguments: edge.expressions.continuation_arguments,
                guard_expression,
                guard_has_expression: guard_expression.is_valid(),
                guard_lowering: guard_comparison.lowering,
                guard_operator: guard_comparison.operator,
                guard_storage: guard_comparison.storage,
                guard_byte_offset: guard_comparison.byte_offset,
                guard_right_storage: guard_comparison.right_storage,
                guard_right_byte_offset: guard_comparison.right_byte_offset,
                guard_byte_size: guard_comparison.byte_size,
                guard_expected_value: guard_comparison.expected_value,
                guard_has_storage: guard_comparison.has_storage,
                guard_has_right_storage: guard_comparison.has_right_storage,
                action: dispatch_action(&edge.target),
                forms_cycle: edge.forms_cycle,
            }
        })
        .collect();

    CollectedRuntimeDispatchLoopCase {
        key: state.key,
        dispatch_index: state.dispatch_index,
        operation_count: runtime_body_operation_count(context, state.dispatch_index),
        edges,
    }
}

fn dispatch_action(target: &RuntimeTransitionTarget) -> RuntimeDispatchLoopAction {
    match target {
        RuntimeTransitionTarget::State { .. } => RuntimeDispatchLoopAction::EnterState,
        RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
            RuntimeDispatchLoopAction::Terminate
        }
        RuntimeTransitionTarget::Unknown { .. } => RuntimeDispatchLoopAction::Unknown,
    }
}

fn runtime_body_operation_count(
    context: &RuntimeDispatchLoopContext,
    dispatch_index: u32,
) -> usize {
    context
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.dispatch_index == dispatch_index)
        .and_then(|(_, body)| {
            context
                .runtime_bodies
                .operations
                .paged_span(body.operations)
        })
        .map(|operations| operations.len())
        .unwrap_or(0)
}
