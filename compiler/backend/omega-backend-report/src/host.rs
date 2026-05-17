use super::backend_state_name;

use crate::BackendReportInput;
use omega_calling_conventions::{
    HostBinding, HostBindingMechanism, PlatformCallData, PlatformCallLowering,
};
use omega_platform_interface::{
    HostCall, HostCallArgument, HostCallArgumentKind, LoweredHostOperation,
};

pub(super) fn write_host_sections(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    output.push_str("## Host ABI\n");
    output.push_str(&format!(
        "bindings: {}\n",
        backend_plan.host_abi.bindings.len()
    ));
    for (_, binding) in backend_plan.host_abi.bindings.iter() {
        write_host_binding(output, binding);
    }
    output.push_str(&format!(
        "platform lowerings: {}\n",
        backend_plan.host_abi.platform_call_lowerings.len()
    ));
    for (_, lowering) in backend_plan.host_abi.platform_call_lowerings.iter() {
        write_platform_call_lowering(output, backend_plan, lowering);
    }
    output.push('\n');

    output.push_str("## Host Call Lowering\n");
    output.push_str(&format!("calls: {}\n", backend_plan.host_calls.calls.len()));
    output.push_str(&format!(
        "unsupported calls: {}\n",
        backend_plan.host_calls.unsupported_calls.len()
    ));
    output.push_str(&format!(
        "operations: {}\n",
        backend_plan.host_calls.operations.len()
    ));
    if backend_plan.host_calls.calls.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, call) in backend_plan.host_calls.calls.iter() {
            write_host_call(output, backend_plan, call);
        }
    }
    if !backend_plan.host_calls.unsupported_calls.is_empty() {
        output.push_str("unsupported:\n");
        for (_, unsupported_call) in backend_plan.host_calls.unsupported_calls.iter() {
            let source_name = backend_state_name(backend_plan, unsupported_call.source_key);
            output.push_str(&format!(
                "- {} statement {} `{}`: {}\n",
                source_name,
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason
            ));
        }
    }
    output.push('\n');
}

fn write_host_binding(output: &mut String, binding: &HostBinding) {
    match &binding.mechanism {
        HostBindingMechanism::Import { library, symbol } => {
            output.push_str(&format!(
                "- {}.{} import {}!{} trust `{}`\n",
                binding.operation_key.capability_name(),
                binding.operation_key.operation_name(),
                library,
                symbol,
                binding.trust_policy
            ));
        }
        HostBindingMechanism::Syscall {
            name,
            number,
            number_register,
            supervisor_call,
        } => {
            output.push_str(&format!(
                "- {}.{} syscall {}({}) register x{} svc #{} trust `{}`\n",
                binding.operation_key.capability_name(),
                binding.operation_key.operation_name(),
                name,
                number,
                number_register,
                supervisor_call,
                binding.trust_policy
            ));
        }
    }
}

fn write_platform_call_lowering(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    lowering: &PlatformCallLowering,
) {
    let operations = backend_plan
        .host_abi
        .host_operations
        .span(lowering.operations)
        .map(|operations| {
            operations
                .iter()
                .map(|operation| {
                    format!(
                        "{}.{}",
                        operation.key.capability_name(),
                        operation.key.operation_name()
                    )
                })
                .collect::<Vec<_>>()
                .join(" -> ")
        })
        .unwrap_or_else(|| "invalid operation span".to_owned());

    output.push_str(&format!(
        "- {}.{} => {}",
        lowering.platform, lowering.state, operations
    ));
    match lowering.data {
        PlatformCallData::None => {}
        PlatformCallData::FirstTextArgument { append_newline } => output.push_str(&format!(
            " data first_text_argument append_newline={append_newline}"
        )),
        PlatformCallData::MutableOutputBuffer { byte_capacity } => output.push_str(&format!(
            " data mutable_output_buffer byte_capacity={byte_capacity}"
        )),
    }
    output.push('\n');
}

fn write_host_call(output: &mut String, backend_plan: &BackendReportInput<'_>, call: &HostCall) {
    let source_name = backend_state_name(backend_plan, call.source_key);
    let platform_call = host_call_display_name(backend_plan, call);
    output.push_str(&format!(
        "- {} statement {} `{}`\n",
        source_name, call.statement_index, platform_call
    ));

    match backend_plan.host_calls.arguments.span(call.arguments) {
        Some(arguments) if arguments.is_empty() => output.push_str("  arguments: none\n"),
        Some(arguments) => {
            output.push_str("  arguments:\n");
            for argument in arguments {
                write_host_call_argument(output, backend_plan, argument);
            }
        }
        None => output.push_str("  arguments: invalid span\n"),
    }

    match backend_plan.host_calls.operations.span(call.operations) {
        Some(operations) if operations.is_empty() => output.push_str("  operations: none\n"),
        Some(operations) => {
            output.push_str("  operations:\n");
            for operation in operations {
                write_lowered_host_operation(output, operation);
            }
        }
        None => output.push_str("  operations: invalid span\n"),
    }
}

pub(super) fn host_call_display_name(
    backend_plan: &BackendReportInput<'_>,
    call: &HostCall,
) -> String {
    if !backend_plan
        .host_abi
        .platform_call_lowerings
        .is_valid(call.lowering)
    {
        return "<unknown>".to_owned();
    }

    let lowering = backend_plan
        .host_abi
        .platform_call_lowerings
        .get(call.lowering);
    format!("{}.{}", lowering.platform, lowering.state)
}

fn write_host_call_argument(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    argument: &HostCallArgument,
) {
    let argument_name = match &argument.kind {
        HostCallArgumentKind::Text(text) => format!("text {text:?}"),
        HostCallArgumentKind::Integer(value) => format!("integer {value}"),
        HostCallArgumentKind::Expression(expression) => format!(
            "expression {}",
            backend_plan
                .host_calls
                .expressions
                .display_name(*expression)
        ),
    };

    output.push_str(&format!("  - {argument_name}\n"));
}

fn write_lowered_host_operation(output: &mut String, operation: &LoweredHostOperation) {
    output.push_str(&format!(
        "  - {}.{}\n",
        operation.operation_key.capability_name(),
        operation.operation_key.operation_name()
    ));
}
