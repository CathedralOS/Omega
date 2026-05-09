use crate::host_calls::HostCallArgumentKind;
use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::plan::NativePlan;

pub(in crate::identity) fn count_host_call_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, call) in native_plan.host_calls.calls.iter() {
        storage.count_program_name_identity(&call.machine);
        storage.count_program_name_identity(&call.state);
        storage.count_identity(&call.platform_call);
    }
    for (_, unsupported) in native_plan.host_calls.unsupported_calls.iter() {
        storage.count_program_name_identity(&unsupported.machine);
        storage.count_program_name_identity(&unsupported.state);
        storage.count_identity(&unsupported.platform_call);
        storage.count_report(&unsupported.reason);
    }
    for (_, operation) in native_plan.host_calls.operations.iter() {
        storage.count_identity(&operation.capability);
        storage.count_identity(&operation.operation);
    }
    for (_, argument) in native_plan.host_calls.arguments.iter() {
        match &argument.kind {
            HostCallArgumentKind::Text(value) => storage.count_payload(value),
            HostCallArgumentKind::Expression(expression) => {
                count_expression_strings(expression, storage);
            }
            HostCallArgumentKind::Integer(_) => {}
        }
    }
}

pub(in crate::identity) fn count_state_call_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, call) in native_plan.state_calls.calls.iter() {
        storage.count_program_name_identity(&call.receiver);
        storage.count_program_name_identity(&call.target_state);
    }
    for (_, argument) in native_plan.state_calls.arguments.iter() {
        storage.count_program_name_identity(&argument.parameter_name);
        count_expression_strings(&argument.expression, storage);
    }
}

pub(in crate::identity) fn count_alias_flow_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, alias) in native_plan.alias_flow.aliases.iter() {
        storage.count_program_name_identity(&alias.parameter_name);
        count_expression_strings(&alias.argument, storage);
    }
}
