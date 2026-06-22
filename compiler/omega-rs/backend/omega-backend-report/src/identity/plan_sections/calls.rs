use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_control_flow_expression_strings;
use omega_platform_interface::{HostCallArgumentKind, UnsupportedHostCallReason};

pub(in crate::identity) fn count_host_call_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, unsupported) in backend_plan.host_calls.unsupported_calls.iter() {
        storage.count_identity(&unsupported.platform_call);
        storage.count_report(&unsupported_host_call_reason_text(unsupported.reason));
    }
    for (_, argument) in backend_plan.host_calls.arguments.iter() {
        match &argument.kind {
            HostCallArgumentKind::Text(value) => storage.count_payload(&value),
            HostCallArgumentKind::Expression(expression) => {
                count_control_flow_expression_strings(
                    &backend_plan.host_calls.expressions,
                    *expression,
                    storage,
                );
            }
            HostCallArgumentKind::Integer(_) => {}
        }
    }
}

fn unsupported_host_call_reason_text(reason: UnsupportedHostCallReason) -> String {
    match reason {
        UnsupportedHostCallReason::NoNativeLowering { target } => {
            format!("no native lowering for target {target:?}")
        }
    }
}

pub(in crate::identity) fn count_state_call_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, argument) in backend_plan.state_calls.arguments.iter() {
        storage.count_program_name_identity(&argument.parameter_name);
        count_control_flow_expression_strings(
            &backend_plan.state_calls.expressions,
            argument.expression,
            storage,
        );
    }
}

pub(in crate::identity) fn count_alias_flow_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, alias) in backend_plan.alias_flow.aliases.iter() {
        storage.count_program_name_identity(&alias.parameter_name);
        count_control_flow_expression_strings(
            &backend_plan.alias_flow.expressions,
            alias.argument,
            storage,
        );
    }
}
