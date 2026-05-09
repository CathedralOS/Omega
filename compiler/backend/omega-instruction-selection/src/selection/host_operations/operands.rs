use crate::InstructionSelectionInput;
use omega_calling_conventions::{HostCapability, HostOperation, HostOperationKey};
use omega_platform_interface::{HostCall, HostCallArgument, HostCallArgumentKind};
use omega_target::ObjectFormat;

use super::runtime_text::{
    find_runtime_text_input_buffer_data_object, runtime_machine_string_descriptor_offset,
    runtime_text_literal_for_host_call,
};
use omega_core::arena::Handle;
use omega_target_program::{
    InstructionOperand, InstructionOperandKind, TargetDataObject, TargetDataObjectHandle,
};

pub(super) fn select_host_operation_operands(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    operation_key: HostOperationKey,
) -> Vec<InstructionOperand> {
    match (
        input.target.object_format,
        operation_key.capability,
        operation_key.operation,
    ) {
        (ObjectFormat::Coff, HostCapability::Stdout, HostOperation::GetStdHandle) => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-11))]
        }
        (ObjectFormat::Coff, HostCapability::Stdin, HostOperation::GetStdHandle) => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-10))]
        }
        (_, HostCapability::Stdin, HostOperation::Read | HostOperation::ReadFile) => {
            let Some((data_object_handle, data_object)) = find_data_object(input, host_call) else {
                return Vec::new();
            };
            let byte_count = input
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            let mut operands = Vec::new();
            if operation_key.operation == HostOperation::Read {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(0)));
            }
            operands.push(operand(InstructionOperandKind::DataAddress {
                data: data_object_handle,
            }));
            operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
            operands
        }
        (_, HostCapability::Stdout, HostOperation::Write | HostOperation::WriteFile) => {
            let mut operands = Vec::new();
            if operation_key.operation == HostOperation::Write {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(1)));
            }

            if let Some((data_object_handle, data_object)) = find_data_object(input, host_call) {
                let byte_count = input
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .map_or(0, |bytes| bytes.len());
                operands.push(operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle,
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
                return operands;
            }

            if let Some(data_object) = find_runtime_text_input_buffer_data_object(input, host_call)
                && let Some(literal) = runtime_text_literal_for_host_call(input, host_call)
                && runtime_machine_string_descriptor_offset(input, host_call).is_none()
            {
                operands.push(operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle(input, data_object),
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(literal.len())));
                return operands;
            }

            if let Some(byte_offset) = runtime_machine_string_descriptor_offset(input, host_call) {
                operands.push(operand(
                    InstructionOperandKind::RuntimeMachineStringPointer { byte_offset },
                ));
                operands.push(operand(
                    InstructionOperandKind::RuntimeMachineStringLength { byte_offset },
                ));
                return operands;
            }

            operands
        }
        (
            _,
            HostCapability::Process,
            HostOperation::Exit | HostOperation::ExitGroup | HostOperation::ExitProcess,
        ) => {
            vec![operand(InstructionOperandKind::ImmediateInteger(
                exit_code(host_call, input),
            ))]
        }
        _ => Vec::new(),
    }
}

pub(super) fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
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
