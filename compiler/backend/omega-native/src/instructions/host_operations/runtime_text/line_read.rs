use super::bindings::host_binding_mechanism;
use super::buffers::text_expression_for_buffer_target;
use crate::abi::{HostBindingMechanism, PlatformCallData};
use crate::host_calls::HostCall;
use crate::instructions::model::SelectedInstructionKind;
use crate::instructions::storage_places::{
    resolve_machine_owned_place, resolve_runtime_storage_place,
};
use crate::plan::NativePlan;

pub(in crate::instructions::host_operations) fn runtime_text_line_read(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<SelectedInstructionKind> {
    let PlatformCallData::MutableOutputBuffer { byte_capacity } = host_call.data else {
        return None;
    };
    let Some(HostBindingMechanism::Syscall {
        number: syscall_number,
        number_register: syscall_number_register,
        supervisor_call,
        ..
    }) = host_binding_mechanism(native_plan, "Stdin", "read")
    else {
        return None;
    };

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            buffer.source_key == host_call.source_key
                && buffer.statement_index == host_call.statement_index
        })
        .map(|(_, buffer)| buffer)?;
    let data_object = native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)?;
    let text_place = text_expression_for_buffer_target(&buffer.target)?;
    let target_place = resolve_runtime_storage_place(
        native_plan,
        0,
        host_call.source_key,
        &host_call.machine,
        &host_call.state,
        &text_place,
    )?;
    if target_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    Some(SelectedInstructionKind::ReadRuntimeTextLine {
        buffer_symbol: data_object.symbol.clone(),
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_capacity,
        syscall_number: *syscall_number,
        syscall_number_register: *syscall_number_register,
        supervisor_call: *supervisor_call,
    })
}

pub(in crate::instructions) fn runtime_machine_string_descriptor_offset(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<usize> {
    let first_argument = native_plan
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())?;
    let crate::host_calls::HostCallArgumentKind::Expression(expression) = &first_argument.kind
    else {
        return None;
    };
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        native_plan.entry_machine_name(),
        &host_call.machine,
        expression,
    )?;
    (byte_size == native_plan.target.pointer_size * 2).then_some(byte_offset)
}
