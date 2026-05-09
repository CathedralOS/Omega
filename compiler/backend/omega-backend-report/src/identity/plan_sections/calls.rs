use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_expression_strings;
use omega_platform_interface::HostCallArgumentKind;

pub(in crate::identity) fn count_host_call_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, call) in backend_plan.host_calls.calls.iter() {
        storage.count_identity(&call.platform_call);
    }
    for (_, unsupported) in backend_plan.host_calls.unsupported_calls.iter() {
        storage.count_identity(&unsupported.platform_call);
        storage.count_report(&unsupported.reason);
    }
    for (_, argument) in backend_plan.host_calls.arguments.iter() {
        match &argument.kind {
            HostCallArgumentKind::Text(value) => storage.count_payload(&value),
            HostCallArgumentKind::Expression(expression) => {
                count_expression_strings(&expression, storage);
            }
            HostCallArgumentKind::Integer(_) => {}
        }
    }
}

pub(in crate::identity) fn count_state_call_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, call) in backend_plan.state_calls.calls.iter() {
        storage.count_program_name_report(&call.receiver_display);
    }
    for (_, argument) in backend_plan.state_calls.arguments.iter() {
        storage.count_program_name_identity(&argument.parameter_name);
        count_expression_strings(&argument.expression, storage);
    }
}

pub(in crate::identity) fn count_alias_flow_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, alias) in backend_plan.alias_flow.aliases.iter() {
        storage.count_program_name_identity(&alias.parameter_name);
        count_expression_strings(&alias.argument, storage);
    }
}
