use super::buffers::find_runtime_text_input_buffer_data;
use crate::InstructionSelectionInput;
use omega_calling_conventions::PlatformCallData;
use omega_control_flow::StateKey;
use omega_platform_interface::HostCall;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_runtime_text::RuntimeTextWriteKind;
use omega_abstract_operations::TargetDataObjectHandle;
use std::sync::Arc;

pub(in crate::selection) struct RuntimeTextLiteralWrite {
    pub buffer: TargetDataObjectHandle,
    pub literal: Arc<str>,
}

pub(in crate::selection) fn runtime_text_literal_write_for_host_call(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> Option<RuntimeTextLiteralWrite> {
    let literal = runtime_text_literal_for_host_call(input, host_call)?;
    let buffer = find_runtime_text_input_buffer_data(input, host_call);
    if !buffer.is_valid() {
        return None;
    }
    Some(RuntimeTextLiteralWrite { buffer, literal })
}

pub(in crate::selection::host_operations) fn runtime_text_literal_for_host_call(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> Option<Arc<str>> {
    let append_newline = match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => append_newline,
        PlatformCallData::MutableOutputBuffer { .. } | PlatformCallData::None => return None,
    };
    if !host_call_uses_runtime_text_input_buffer(input, host_call) {
        return None;
    }

    let runtime_body = input
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| {
            input
                .runtime_bodies
                .operations
                .span(body.operations)
                .is_some_and(|operations| {
                    operations.iter().any(|operation| {
                        operation.source_key == host_call.source_key
                            && operation.statement_index == host_call.statement_index
                            && matches!(operation.kind, RuntimeDispatchBodyOperationKind::HostCall)
                    })
                })
        })
        .map(|(_, body)| body)?;
    let operations = input
        .runtime_bodies
        .operations
        .span(runtime_body.operations)?;
    let mut latest_static_text = None;

    for operation in operations {
        if operation.source_key == host_call.source_key
            && operation.statement_index == host_call.statement_index
            && matches!(operation.kind, RuntimeDispatchBodyOperationKind::HostCall)
        {
            break;
        }

        let Some(text_write) = runtime_text_write_for_operation(
            input,
            operation.source_key,
            operation.statement_index,
        ) else {
            continue;
        };
        if text_write.kind != RuntimeTextWriteKind::StaticText {
            continue;
        }
        let Some(value) = input
            .runtime_text
            .expressions
            .string_literal_value(text_write.value)
        else {
            continue;
        };
        latest_static_text = Some(value);
    }

    let literal = latest_static_text?;
    if append_newline {
        let mut literal = String::from(literal.as_ref());
        literal.push('\n');
        return Some(Arc::from(literal));
    }

    Some(literal)
}

fn host_call_uses_runtime_text_input_buffer(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> bool {
    find_runtime_text_input_buffer_data(input, host_call).is_valid()
}

fn runtime_text_write_for_operation<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan omega_runtime_text::RuntimeTextWrite> {
    input
        .runtime_text
        .writes
        .iter()
        .find(|(_, write)| {
            write.source_key == source_key && write.statement_index == statement_index
        })
        .map(|(_, write)| write)
}
