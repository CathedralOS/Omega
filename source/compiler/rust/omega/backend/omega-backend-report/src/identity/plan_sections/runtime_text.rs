use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_control_flow_expression_strings;

pub(in crate::identity) fn count_runtime_text_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, text_use) in backend_plan.runtime_text.uses.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            text_use.expression,
            storage,
        );
    }
    for (_, buffer) in backend_plan.runtime_text.buffers.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            buffer.target,
            storage,
        );
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            buffer.text_place,
            storage,
        );
    }
    for (_, slot) in backend_plan.runtime_text.slots.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            slot.place,
            storage,
        );
    }
    for (_, write) in backend_plan.runtime_text.writes.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            write.target,
            storage,
        );
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            write.value,
            storage,
        );
    }
    for (_, builder) in backend_plan.runtime_text.builders.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            builder.target,
            storage,
        );
    }
    for (_, segment) in backend_plan.runtime_text.builder_segments.iter() {
        count_control_flow_expression_strings(
            &backend_plan.runtime_text.expressions,
            segment.expression,
            storage,
        );
    }
}
