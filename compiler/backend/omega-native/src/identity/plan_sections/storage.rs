use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::plan::NativePlan;

pub(in crate::identity) fn count_state_storage_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, local) in native_plan.state_storage.locals.iter() {
        storage.count_program_name_identity(&local.name);
        storage.count_identity(&local.type_name);
    }
    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        count_expression_strings(&mutation.target, storage);
        count_expression_strings(&mutation.value, storage);
    }
}

pub(in crate::identity) fn count_state_value_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, value) in native_plan.state_values.values.iter() {
        count_expression_strings(&value.expression, storage);
    }
}

pub(in crate::identity) fn count_runtime_storage_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
        storage.count_program_name_identity(&slot.name);
        storage.count_identity(&slot.type_name);
    }
    for (_, write) in native_plan.runtime_storage.writes.iter() {
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
}
