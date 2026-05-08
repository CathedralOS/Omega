use crate::abi::{HostBindingMechanism, PlatformCallData};
use crate::control_flow::StateKey;
use crate::data::NativeDataObject;
use crate::host_calls::{HostCall, HostCallArgumentKind};
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_text::{RuntimeTextSource, RuntimeTextWriteKind};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::super::model::SelectedInstructionKind;
use super::super::storage_places::{resolve_machine_owned_place, resolve_runtime_storage_place};

pub(super) fn runtime_text_line_read(
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

pub(super) fn find_runtime_text_input_buffer_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    let text_use = native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.source_key == host_call.source_key
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
                && text_use.source == RuntimeTextSource::StoredPlace
        })
        .map(|(_, text_use)| text_use)?;

    let text_slot = native_plan
        .runtime_text
        .slots
        .iter()
        .find(|(_, slot)| {
            slot.place.display_name() == text_use.expression.display_name() && slot.has_input_buffer
        })
        .map(|(_, slot)| slot)?;

    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_slot.place.display_name())
        })
        .map(|(_, buffer)| buffer)?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
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
    let HostCallArgumentKind::Expression(expression) = &first_argument.kind else {
        return None;
    };
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &host_call.machine,
        expression,
    )?;
    (byte_size == native_plan.target.pointer_size * 2).then_some(byte_offset)
}

pub(in crate::instructions) fn runtime_text_input_buffer_for_text_place<'plan>(
    native_plan: &'plan NativePlan,
    text_place: &Expression,
) -> Option<&'plan NativeDataObject> {
    let text_place_name = text_place.display_name();
    let buffer = native_plan
        .runtime_text
        .buffers
        .iter()
        .find_map(|(_, buffer)| {
            text_place_for_buffer_target(&buffer.target)
                .is_some_and(|place_name| place_name == text_place_name)
                .then_some(buffer)
        })?;

    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(_, data_object)| data_object)
}

pub(in crate::instructions) fn runtime_text_literal_write_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<(String, String)> {
    let literal = runtime_text_literal_for_host_call(native_plan, host_call)?;
    let data_object = find_runtime_text_input_buffer_data_object(native_plan, host_call)?;
    Some((data_object.symbol.clone(), literal))
}

pub(super) fn runtime_text_literal_for_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> Option<String> {
    let append_newline = match host_call.data {
        PlatformCallData::FirstTextArgument { append_newline } => append_newline,
        PlatformCallData::MutableOutputBuffer { .. } | PlatformCallData::None => return None,
    };
    if !host_call_uses_runtime_text_input_buffer(native_plan, host_call) {
        return None;
    }

    let runtime_body = native_plan
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| {
            native_plan
                .runtime_bodies
                .operations
                .span(body.operations)
                .is_some_and(|operations| {
                    operations.iter().any(|operation| {
                        operation.source_key == host_call.source_key
                            && operation.statement_index == host_call.statement_index
                            && matches!(
                                operation.kind,
                                RuntimeDispatchBodyOperationKind::HostCall { .. }
                            )
                    })
                })
        })
        .map(|(_, body)| body)?;
    let operations = native_plan
        .runtime_bodies
        .operations
        .span(runtime_body.operations)?;
    let mut latest_static_text = None;

    for operation in operations {
        if operation.source_key == host_call.source_key
            && operation.statement_index == host_call.statement_index
            && matches!(
                operation.kind,
                RuntimeDispatchBodyOperationKind::HostCall { .. }
            )
        {
            break;
        }

        let Some(text_write) = runtime_text_write_for_operation(
            native_plan,
            operation.source_key,
            operation.statement_index,
        ) else {
            continue;
        };
        if text_write.kind != RuntimeTextWriteKind::StaticText {
            continue;
        }
        let Expression::String(value) = &text_write.value else {
            continue;
        };
        latest_static_text = Some(value.clone());
    }

    let mut literal = latest_static_text?;
    if append_newline {
        literal.push('\n');
    }
    Some(literal)
}

fn host_binding_mechanism<'plan>(
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBindingMechanism> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| &binding.mechanism)
}

fn text_place_for_buffer_target(target: &Expression) -> Option<String> {
    text_expression_for_buffer_target(target).map(|expression| expression.display_name())
}

fn text_expression_for_buffer_target(target: &Expression) -> Option<Expression> {
    match target {
        Expression::Name(path) => {
            let mut text_path = path.clone();
            text_path.push(ProgramName::generated("text"));
            Some(Expression::Name(text_path))
        }
        _ => None,
    }
}

fn host_call_uses_runtime_text_input_buffer(
    native_plan: &NativePlan,
    host_call: &HostCall,
) -> bool {
    find_runtime_text_input_buffer_data_object(native_plan, host_call).is_some()
}

fn runtime_text_write_for_operation<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan crate::runtime_text::RuntimeTextWrite> {
    native_plan
        .runtime_text
        .writes
        .iter()
        .find(|(_, write)| {
            write.source_key == source_key && write.statement_index == statement_index
        })
        .map(|(_, write)| write)
}
