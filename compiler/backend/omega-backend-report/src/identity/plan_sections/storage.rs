use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_control_flow_expression_strings;

pub(in crate::identity) fn count_state_storage_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, local) in backend_plan.state_storage.locals.iter() {
        storage.count_program_name_identity(&local.name);
        storage.count_identity(
            &backend_plan
                .state_storage
                .type_references
                .display_name(local.type_reference),
        );
    }
    for (_, mutation) in backend_plan.state_storage.mutations.iter() {
        count_control_flow_expression_strings(
            &backend_plan.state_storage.expressions,
            mutation.target,
            storage,
        );
        count_control_flow_expression_strings(
            &backend_plan.state_storage.expressions,
            mutation.value,
            storage,
        );
    }
}

pub(in crate::identity) fn count_state_value_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, value) in backend_plan.state_values.values.iter() {
        count_control_flow_expression_strings(
            &backend_plan.state_values.expressions,
            value.expression,
            storage,
        );
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
        count_control_flow_expression_strings(
            &backend_plan.runtime_storage.expressions,
            write.target,
            storage,
        );
        count_control_flow_expression_strings(
            &backend_plan.runtime_storage.expressions,
            write.value,
            storage,
        );
    }
}
