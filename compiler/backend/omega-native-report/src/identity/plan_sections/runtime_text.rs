use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use omega_backend_plan::NativePlan;

pub(in crate::identity) fn count_runtime_text_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, text_use) in native_plan.runtime_text.uses.iter() {
        storage.count_identity(&text_use.platform_call);
        count_expression_strings(&text_use.expression, storage);
    }
    for (_, buffer) in native_plan.runtime_text.buffers.iter() {
        storage.count_identity(&buffer.platform_call);
        count_expression_strings(&buffer.target, storage);
    }
    for (_, slot) in native_plan.runtime_text.slots.iter() {
        count_expression_strings(&slot.place, storage);
    }
    for (_, write) in native_plan.runtime_text.writes.iter() {
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
    for (_, builder) in native_plan.runtime_text.builders.iter() {
        count_expression_strings(&builder.target, storage);
    }
    for (_, segment) in native_plan.runtime_text.builder_segments.iter() {
        count_expression_strings(&segment.expression, storage);
    }
}
