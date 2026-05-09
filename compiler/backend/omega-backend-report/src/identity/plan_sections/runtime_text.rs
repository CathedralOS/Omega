use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_expression_strings;

pub(in crate::identity) fn count_runtime_text_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, text_use) in backend_plan.runtime_text.uses.iter() {
        storage.count_identity(&text_use.platform_call);
        count_expression_strings(&text_use.expression, storage);
    }
    for (_, buffer) in backend_plan.runtime_text.buffers.iter() {
        storage.count_identity(&buffer.platform_call);
        count_expression_strings(&buffer.target, storage);
    }
    for (_, slot) in backend_plan.runtime_text.slots.iter() {
        count_expression_strings(&slot.place, storage);
    }
    for (_, write) in backend_plan.runtime_text.writes.iter() {
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
    for (_, builder) in backend_plan.runtime_text.builders.iter() {
        count_expression_strings(&builder.target, storage);
    }
    for (_, segment) in backend_plan.runtime_text.builder_segments.iter() {
        count_expression_strings(&segment.expression, storage);
    }
}
