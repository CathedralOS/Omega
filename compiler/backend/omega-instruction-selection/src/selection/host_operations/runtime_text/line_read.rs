use super::bindings::host_binding_mechanism;
use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    resolve_machine_owned_place, resolve_runtime_storage_place,
};
use omega_calling_conventions::{
    HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, PlatformCallData,
};
use omega_platform_interface::HostCall;
use omega_target_program::{RuntimeTextReadSource, SelectedInstructionKind};
use omega_typed_program::name::ProgramName;

pub(in crate::selection::host_operations) fn runtime_text_line_read(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> Option<SelectedInstructionKind> {
    let PlatformCallData::MutableOutputBuffer { byte_capacity } = host_call.data else {
        return None;
    };
    let Some(read_source) = runtime_text_read_source(input) else {
        return None;
    };

    let buffer = input
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            buffer.source_key == host_call.source_key
                && buffer.statement_index == host_call.statement_index
        })
        .map(|(_, buffer)| buffer)?;
    let (data_object, _) = input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(data, data_object)| (data, data_object))?;
    let text_suffix = ProgramName::generated("text");
    let text_place = input
        .runtime_text
        .expressions
        .to_tree_with_place_suffix(buffer.target, std::slice::from_ref(&text_suffix));
    let source_machine = input
        .control_flow
        .state_machine_name_by_key_cloned(host_call.source_key);
    let source_state = input
        .control_flow
        .state_name_by_key_cloned(host_call.source_key);
    let target_place = resolve_runtime_storage_place(
        input,
        0,
        host_call.source_key,
        &source_machine,
        &source_state,
        &text_place,
    )?;
    if target_place.byte_count != input.target.pointer_size * 2 {
        return None;
    }

    Some(SelectedInstructionKind::ReadRuntimeTextLine {
        buffer: data_object,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_capacity,
        source: read_source,
    })
}

fn runtime_text_read_source(
    input: &InstructionSelectionInput<'_>,
) -> Option<RuntimeTextReadSource> {
    match host_binding_mechanism(
        input,
        HostOperationKey::new(HostCapability::Stdin, HostOperation::Read),
    )? {
        HostBindingMechanism::Import { symbol, .. } => Some(RuntimeTextReadSource::Import {
            symbol: symbol.clone(),
        }),
        HostBindingMechanism::Syscall {
            number,
            number_register,
            supervisor_call,
            ..
        } => Some(RuntimeTextReadSource::Syscall {
            number: *number,
            number_register: *number_register,
            supervisor_call: *supervisor_call,
        }),
    }
}

pub(in crate::selection) fn runtime_machine_string_descriptor_offset(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
) -> Option<usize> {
    let first_argument = input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())?;
    let omega_platform_interface::HostCallArgumentKind::Expression(expression) =
        &first_argument.kind
    else {
        return None;
    };
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &input.layouts,
        input.entry_key.machine,
        host_call.source_key.machine,
        expression,
    )?;
    (byte_size == input.target.pointer_size * 2).then_some(byte_offset)
}
