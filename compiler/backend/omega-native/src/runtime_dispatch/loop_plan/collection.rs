use super::context::RuntimeDispatchLoopContext;
use super::guards::dispatch_guard_comparison;
use super::inputs::RuntimeDispatchLoopCaseInput;
use super::model::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_control_flow::StateKey;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchLoopCase {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub label: String,
    pub operation_count: usize,
    pub edges: Vec<RuntimeDispatchLoopEdge>,
}

pub(super) fn build_runtime_dispatch_loop_case(
    context: &RuntimeDispatchLoopContext,
    case_input: &RuntimeDispatchLoopCaseInput,
) -> CollectedRuntimeDispatchLoopCase {
    let edges = case_input
        .edges
        .iter()
        .enumerate()
        .map(|(order, edge)| {
            let guard_comparison =
                dispatch_guard_comparison(context, case_input.dispatch_index, order);
            RuntimeDispatchLoopEdge {
                order,
                target: edge.target.clone(),
                target_dispatch_index: edge.target_dispatch_index,
                continuation: edge.continuation.clone(),
                continuation_dispatch_index: edge.continuation_dispatch_index,
                guard: edge.guard.clone(),
                guard_lowering: guard_comparison.lowering,
                guard_operator: guard_comparison.operator,
                guard_byte_offset: guard_comparison.byte_offset,
                guard_byte_size: guard_comparison.byte_size,
                guard_expected_value: guard_comparison.expected_value,
                guard_has_storage: guard_comparison.has_storage,
                action: dispatch_action(&edge.target),
                forms_cycle: edge.forms_cycle,
            }
        })
        .collect();

    CollectedRuntimeDispatchLoopCase {
        key: case_input.key,
        dispatch_index: case_input.dispatch_index,
        label: case_input.label.clone(),
        operation_count: runtime_body_operation_count(context, case_input.dispatch_index),
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
