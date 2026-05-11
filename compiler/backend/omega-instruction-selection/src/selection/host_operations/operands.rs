use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasResolutionContext;
use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
use omega_platform_interface::{HostCall, HostCallArgument, HostCallArgumentKind};
use omega_target::ObjectFormat;

use super::runtime_text::{
    find_runtime_text_input_buffer_data_object, runtime_string_descriptor_place,
    runtime_text_literal_for_host_call,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target_operations::{
    InstructionOperand, InstructionOperandKind, TargetDataObject, TargetDataObjectHandle,
};

pub(super) fn select_host_operation_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    operation_key: HostOperationKey,
    operands: &mut Arena<InstructionOperand>,
) -> HandleSpan<InstructionOperand> {
    match (
        input.target.object_format,
        operation_key.capability,
        operation_key.operation,
    ) {
        (ObjectFormat::Coff, HostCapability::Stdout, HostOperation::GetStdHandle) => {
            operands.insert_many([operand(InstructionOperandKind::ImmediateInteger(-11))])
        }
        (ObjectFormat::Coff, HostCapability::Stdin, HostOperation::GetStdHandle) => {
            operands.insert_many([operand(InstructionOperandKind::ImmediateInteger(-10))])
        }
        (_, HostCapability::Stdin, HostOperation::Read | HostOperation::ReadFile) => {
            let Some((data_object_handle, data_object)) = find_data_object(input, host_call) else {
                return HandleSpan::empty();
            };
            let byte_count = input
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            if operation_key.operation == HostOperation::Read {
                return operands.insert_many([
                    operand(InstructionOperandKind::ImmediateInteger(0)),
                    operand(InstructionOperandKind::DataAddress {
                        data: data_object_handle,
                    }),
                    operand(InstructionOperandKind::ByteLength(byte_count)),
                ]);
            }

            operands.insert_many([
                operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle,
                }),
                operand(InstructionOperandKind::ByteLength(byte_count)),
            ])
        }
        (_, HostCapability::Stdout, HostOperation::Write | HostOperation::WriteFile) => {
            if let Some((data_object_handle, data_object)) = find_data_object(input, host_call) {
                let byte_count = input
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .map_or(0, |bytes| bytes.len());
                return stdout_operands(
                    operands,
                    operation_key.operation,
                    InstructionOperandKind::DataAddress {
                        data: data_object_handle,
                    },
                    InstructionOperandKind::ByteLength(byte_count),
                );
            }

            if let Some(data_object) = find_runtime_text_input_buffer_data_object(input, host_call)
                && let Some(literal) = runtime_text_literal_for_host_call(input, host_call)
                && runtime_string_descriptor_place(
                    input,
                    host_call,
                    dispatch_index,
                    alias_context,
                )
                .is_none()
            {
                return stdout_operands(
                    operands,
                    operation_key.operation,
                    InstructionOperandKind::DataAddress {
                        data: data_object_handle(input, data_object),
                    },
                    InstructionOperandKind::ByteLength(literal.len()),
                );
            }

            if let Some(place) = runtime_string_descriptor_place(
                input,
                host_call,
                dispatch_index,
                alias_context,
            )
            {
                return stdout_operands(
                    operands,
                    operation_key.operation,
                    InstructionOperandKind::RuntimeStringPointer {
                        region: place.region,
                        byte_offset: place.byte_offset,
                    },
                    InstructionOperandKind::RuntimeStringLength {
                        region: place.region,
                        byte_offset: place.byte_offset,
                    },
                );
            }

            HandleSpan::empty()
        }
        (
            _,
            HostCapability::Process,
            HostOperation::Exit | HostOperation::ExitGroup | HostOperation::ExitProcess,
        ) => operands.insert_many([operand(InstructionOperandKind::ImmediateInteger(
            exit_code(host_call, input),
        ))]),
        _ => HandleSpan::empty(),
    }
}

pub(super) fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
}

fn stdout_operands(
    operands: &mut Arena<InstructionOperand>,
    operation: HostOperation,
    first: InstructionOperandKind,
    second: InstructionOperandKind,
) -> HandleSpan<InstructionOperand> {
    if operation == HostOperation::Write {
        return operands.insert_many([
            operand(InstructionOperandKind::ImmediateInteger(1)),
            operand(first),
            operand(second),
        ]);
    }

    operands.insert_many([operand(first), operand(second)])
}

fn find_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    host_call: &HostCall,
) -> Option<(TargetDataObjectHandle, &'plan TargetDataObject)> {
    input.data.objects.iter().find(|(_, data_object)| {
        data_object.source_key == host_call.source_key
            && data_object.source_statement == host_call.statement_index
    })
}

pub(super) fn data_object_handle(
    input: &InstructionSelectionInput<'_>,
    target: &TargetDataObject,
) -> TargetDataObjectHandle {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == target.source_key
                && data_object.source_statement == target.source_statement
                && data_object.offset == target.offset
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(Handle::invalid)
}

fn exit_code(host_call: &HostCall, input: &InstructionSelectionInput<'_>) -> i64 {
    first_argument(host_call, input)
        .and_then(|argument| match &argument.kind {
            HostCallArgumentKind::Integer(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(0)
}

fn first_argument<'plan>(
    host_call: &HostCall,
    input: &'plan InstructionSelectionInput<'plan>,
) -> Option<&'plan HostCallArgument> {
    input
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}
