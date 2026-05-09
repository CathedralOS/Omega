use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_expression_strings;

pub(in crate::identity) fn count_state_storage_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, local) in backend_plan.state_storage.locals.iter() {
        storage.count_program_name_identity(&local.name);
        storage.count_identity(&local.type_name);
    }
    for (_, mutation) in backend_plan.state_storage.mutations.iter() {
        count_expression_strings(&mutation.target, storage);
        count_expression_strings(&mutation.value, storage);
    }
}

pub(in crate::identity) fn count_state_value_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, value) in backend_plan.state_values.values.iter() {
        count_expression_strings(&value.expression, storage);
    }
}

pub(in crate::identity) fn count_runtime_storage_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, slot) in backend_plan.runtime_storage.frame_slots.iter() {
        storage.count_program_name_identity(&slot.name);
        storage.count_identity(&slot.type_name);
    }
    for (_, write) in backend_plan.runtime_storage.writes.iter() {
        count_expression_strings(&write.target, storage);
        count_expression_strings(&write.value, storage);
    }
}
