mod operands;
mod runtime_text;

use crate::InstructionSelectionInput;
use omega_calling_conventions::{
    HostCapability, HostOperation, HostOperationKey, PlatformCallData,
};
use omega_core::arena::Arena;
use omega_platform_interface::HostCall;
use omega_target_program::TargetDataObject;

use omega_target_program::{
    InstructionOperand, InstructionOperandKind, SelectedInstruction, SelectedInstructionKind,
};
use operands::{data_object_handle, operand, select_host_operation_operands};
use runtime_text::runtime_text_line_read;
pub(super) use runtime_text::{
    runtime_machine_string_descriptor_offset, runtime_text_input_buffer_data_for_text_place,
    runtime_text_literal_write_for_host_call,
};

pub(super) fn select_host_call(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });

    if let Some(read_line) = runtime_text_line_read(input, host_call) {
        selected_instructions.push(SelectedInstruction {
            kind: read_line,
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
        return;
    }

    let Some(operations) = input.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for operation in operations {
        let operation_operands =
            select_host_operation_operands(input, host_call, operation.operation_key);
        let operation_operands = operands.insert_many(operation_operands);

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                operation_key: operation.operation_key,
                operands: operation_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }

    if host_call_appends_newline(host_call)
        && runtime_machine_string_descriptor_offset(input, host_call).is_some()
        && let Some(newline) = newline_data_object(input)
    {
        let newline_operands = operands.insert_many(vec![
            operand(InstructionOperandKind::ImmediateInteger(1)),
            operand(InstructionOperandKind::DataAddress {
                data: data_object_handle(input, newline),
            }),
            operand(InstructionOperandKind::ByteLength(1)),
        ]);
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                operation_key: HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
                operands: newline_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }
}

fn newline_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
) -> Option<&'plan TargetDataObject> {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| data_object.symbol == "omega_newline")
        .map(|(_, data_object)| data_object)
}

fn host_call_appends_newline(host_call: &HostCall) -> bool {
    matches!(
        host_call.data,
        PlatformCallData::FirstTextArgument {
            append_newline: true
        }
    )
}
