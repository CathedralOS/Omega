mod host_calls;
mod static_strings;

use host_calls::{collect_host_call_data, collect_newline_data, collect_runtime_text_buffer_data};
use omega_calling_conventions::PlatformCallData;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_platform_interface::HostCallArgumentKind;
use omega_platform_interface::HostCallPlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target_operations::TargetDataPlan;
use static_strings::{collect_static_string_assignment_data, collect_static_string_value_data};

pub fn build_target_data_plan(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
    state_values: &StateValuePlan,
    runtime_text: &RuntimeTextPlan,
) -> TargetDataPlan {
    let capacity =
        estimate_target_data_capacity(host_calls, state_storage, state_values, runtime_text);
    let mut data_plan = TargetDataPlan::with_capacity(capacity.objects, capacity.bytes);

    for (_, host_call) in host_calls.calls.iter() {
        collect_host_call_data(host_calls, host_call, &mut data_plan);
    }
    collect_runtime_text_buffer_data(runtime_text, &mut data_plan);
    collect_newline_data(host_calls, &mut data_plan);
    collect_static_string_assignment_data(state_storage, &mut data_plan);
    collect_static_string_value_data(state_values, &mut data_plan);

    data_plan
}

#[derive(Debug, Clone, Copy, Default)]
struct TargetDataCapacity {
    objects: usize,
    bytes: usize,
}

fn estimate_target_data_capacity(
    host_calls: &HostCallPlan,
    state_storage: &StateStoragePlan,
    state_values: &StateValuePlan,
    runtime_text: &RuntimeTextPlan,
) -> TargetDataCapacity {
    let mut capacity = TargetDataCapacity::default();

    estimate_host_call_data_capacity(host_calls, &mut capacity);
    estimate_runtime_text_buffer_capacity(runtime_text, &mut capacity);
    estimate_static_string_capacity_for_state_storage(state_storage, &mut capacity);
    estimate_static_string_capacity_for_state_values(state_values, &mut capacity);

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

fn estimate_static_string_expression_capacity(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    capacity: &mut TargetDataCapacity,
) {
    match expressions.expression(expression) {
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
        ExpressionNode::Binary(binary) => {
            estimate_static_string_expression_capacity(expressions, binary.left, capacity);
            estimate_static_string_expression_capacity(expressions, binary.right, capacity);
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
        | ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_) => {}
    }
}
