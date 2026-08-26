use crate::dynamic_conformances::collect_dynamic_conformance_tables;
use crate::host_calls::{
    collect_host_call_data, collect_newline_data, collect_runtime_text_buffer_data,
};
use crate::static_strings::{
    collect_static_string_assignment_data, collect_static_string_branch_target_data,
    collect_static_string_local_initializer_data, collect_static_string_value_data,
    local_initializer_expression,
};
use omega_calling_conventions::PlatformCallData;
use omega_platform_interface::{HostCallArgumentKind, HostCallPlan};
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target_operations::TargetDataPlan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub fn build_target_data_plan(
    program: &CheckedTrees,
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
    state_values: &StateValuePlan,
    runtime_branching: &RuntimeBranchingCallPlan,
    runtime_text: &RuntimeTextPlan,
) -> TargetDataPlan {
    let capacity = estimate_target_data_capacity(
        program,
        host_calls,
        state_storage,
        state_values,
        runtime_branching,
        runtime_text,
    );
    let mut data_plan = TargetDataPlan::with_capacity(capacity.objects, capacity.bytes);

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }
    collect_runtime_text_buffer_data(runtime_text, &mut data_plan);
    collect_newline_data(host_calls, &mut data_plan);
    collect_static_string_assignment_data(state_storage, &mut data_plan);
    collect_static_string_local_initializer_data(program, state_storage, &mut data_plan);
    collect_static_string_value_data(state_values, &mut data_plan);
    collect_static_string_branch_target_data(runtime_branching, &mut data_plan);

    data_plan
}

pub fn build_target_data_plan_with_dynamic_conformances(
    program: &CheckedTrees,
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
    state_values: &StateValuePlan,
    runtime_branching: &RuntimeBranchingCallPlan,
    runtime_text: &RuntimeTextPlan,
    state_calls: &omega_state_calls::StateCallPlan,
    runtime_abi: omega_runtime_abi::RuntimeAbiPlan,
) -> Result<TargetDataPlan, psi_diagnostics::Diagnostic> {
    let mut data_plan = build_target_data_plan(
        program,
        host_calls,
        state_storage,
        state_values,
        runtime_branching,
        runtime_text,
    );
    collect_dynamic_conformance_tables(program, state_calls, runtime_abi, &mut data_plan)?;
    Ok(data_plan)
}

#[derive(Debug, Clone, Copy, Default)]
struct TargetDataCapacity {
    objects: usize,
    bytes: usize,
}

fn estimate_target_data_capacity(
    program: &CheckedTrees,
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
    state_values: &StateValuePlan,
    runtime_branching: &RuntimeBranchingCallPlan,
    runtime_text: &RuntimeTextPlan,
) -> TargetDataCapacity {
    let mut capacity = TargetDataCapacity::default();

    estimate_host_call_data_capacity(host_calls, &mut capacity);
    estimate_runtime_text_buffer_capacity(runtime_text, &mut capacity);
    estimate_static_string_capacity_for_state_storage(state_storage, &mut capacity);
    estimate_static_string_capacity_for_local_initializers(program, state_storage, &mut capacity);
    estimate_static_string_capacity_for_state_values(state_values, &mut capacity);
    estimate_static_string_capacity_for_runtime_branching(runtime_branching, &mut capacity);

    capacity
}

fn estimate_host_call_data_capacity(host_calls: &HostCallPlan, capacity: &mut TargetDataCapacity) {
    let mut needs_newline_object = false;

    for (_, host_call) in host_calls.calls.iter() {
        let Some(arguments) = host_calls.arguments.span(host_call.arguments) else {
            continue;
        };
        let Some(first_argument) = arguments.first() else {
            continue;
        };

        if let PlatformCallData::FirstTextArgument { append_newline } = host_call.data {
            if append_newline && matches!(first_argument.kind, HostCallArgumentKind::Expression(_))
            {
                needs_newline_object = true;
            }

            if let HostCallArgumentKind::Text(text) = &first_argument.kind {
                capacity.objects = capacity.objects.saturating_add(1);
                capacity.bytes = capacity
                    .bytes
                    .saturating_add(text.len().saturating_add(usize::from(append_newline)));
            }
        }
    }

    if needs_newline_object {
        capacity.objects = capacity.objects.saturating_add(1);
        capacity.bytes = capacity.bytes.saturating_add(1);
    }
}

fn estimate_runtime_text_buffer_capacity(
    runtime_text: &RuntimeTextPlan,
    capacity: &mut TargetDataCapacity,
) {
    for (_, buffer) in runtime_text.buffers.iter() {
        capacity.objects = capacity.objects.saturating_add(1);
        capacity.bytes = capacity.bytes.saturating_add(buffer.byte_capacity);
    }
}

fn estimate_static_string_capacity_for_state_storage(
    state_storage: &StateStoragePlan,
    capacity: &mut TargetDataCapacity,
) {
    for (_, mutation) in state_storage.mutations.iter() {
        if mutation.required {
            estimate_static_string_expression_capacity(
                &state_storage.expressions,
                mutation.value,
                capacity,
            );
        }
    }
}

fn estimate_static_string_capacity_for_local_initializers(
    program: &CheckedTrees,
    state_storage: &StateStoragePlan,
    capacity: &mut TargetDataCapacity,
) {
    for (_, local) in state_storage.locals.iter() {
        if !local.required {
            continue;
        }
        if let Some(initializer) = local_initializer_expression(program, local) {
            estimate_static_string_expression_capacity(
                &program.expression_table,
                initializer,
                capacity,
            );
        }
    }
}

fn estimate_static_string_capacity_for_state_values(
    state_values: &StateValuePlan,
    capacity: &mut TargetDataCapacity,
) {
    for (_, value) in state_values.values.iter() {
        if value.required {
            estimate_static_string_expression_capacity(
                &state_values.expressions,
                value.expression,
                capacity,
            );
        }
    }
}

fn estimate_static_string_capacity_for_runtime_branching(
    runtime_branching: &RuntimeBranchingCallPlan,
    capacity: &mut TargetDataCapacity,
) {
    for (_, expansion) in runtime_branching.leaf_expansions.iter() {
        if expansion.target_value.is_valid() {
            estimate_static_string_expression_capacity(
                &runtime_branching.expressions,
                expansion.target_value,
                capacity,
            );
        }
    }

    for (_, expansion) in runtime_branching.straight_line_expansions.iter() {
        if expansion.target_value.is_valid() {
            estimate_static_string_expression_capacity(
                &runtime_branching.expressions,
                expansion.target_value,
                capacity,
            );
        }
    }
}

fn estimate_static_string_expression_capacity(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    capacity: &mut TargetDataCapacity,
) {
    match expressions.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            estimate_static_string_expression_capacity(expressions, atomic.value, capacity)
        }
        ExpressionNode::String(value) => {
            capacity.objects = capacity.objects.saturating_add(1);
            capacity.bytes = capacity.bytes.saturating_add(value.len().max(1));
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                estimate_static_string_expression_capacity(expressions, field.value, capacity);
            }
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in expressions.expression_handles(*elements) {
                estimate_static_string_expression_capacity(expressions, *element, capacity);
            }
        }
        ExpressionNode::Range(range) => {
            estimate_static_string_expression_capacity(expressions, range.start, capacity);
            estimate_static_string_expression_capacity(expressions, range.end, capacity);
        }
        ExpressionNode::Binary(binary) => {
            estimate_static_string_expression_capacity(expressions, binary.left, capacity);
            estimate_static_string_expression_capacity(expressions, binary.right, capacity);
        }
        ExpressionNode::Unary(unary) => {
            estimate_static_string_expression_capacity(expressions, unary.operand, capacity);
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                estimate_static_string_expression_capacity(expressions, call.receiver, capacity);
            }

            for argument in expressions.expression_handles(call.arguments) {
                estimate_static_string_expression_capacity(expressions, *argument, capacity);
            }
        }
        ExpressionNode::Cast(cast) => {
            estimate_static_string_expression_capacity(expressions, cast.value, capacity);
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_platform_interface::{HostCall, HostCallArgument, HostCallArgumentKind};
    use std::sync::Arc;

    #[test]
    fn host_text_data_retains_non_utf8_bytes_and_newline() {
        let mut host_calls = HostCallPlan::with_capacity(1, 0, 0, 1);
        let mut call = HostCall {
            data: PlatformCallData::FirstTextArgument {
                append_newline: true,
            },
            ..HostCall::default()
        };
        host_calls.arguments.append_to_span(
            &mut call.arguments,
            HostCallArgument {
                kind: HostCallArgumentKind::Text(Arc::from(&[0x80, b'A'][..])),
                ..HostCallArgument::default()
            },
        );
        host_calls.calls.insert(call);

        let data = build_target_data_plan(
            &CheckedTrees::default(),
            &host_calls,
            &StateStoragePlan::default(),
            &StateValuePlan::default(),
            &RuntimeBranchingCallPlan::default(),
            &RuntimeTextPlan::default(),
        );
        let (_, object) = data.objects.iter().next().expect("static host text object");

        assert_eq!(
            data.bytes.span(object.bytes),
            Some(&[0x80, b'A', b'\n'][..])
        );
    }
}
