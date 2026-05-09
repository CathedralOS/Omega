mod operands;
mod runtime_text;

use crate::data::NativeDataObject;
use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use omega_calling_conventions::PlatformCallData;
use omega_core::arena::Arena;

use super::model::{
    InstructionOperand, InstructionOperandKind, SelectedInstruction, SelectedInstructionKind,
};
use operands::{operand, select_host_operation_operands};
use runtime_text::runtime_text_line_read;
pub(super) use runtime_text::{
    runtime_machine_string_descriptor_offset, runtime_text_input_buffer_for_text_place,
    runtime_text_literal_write_for_host_call,
};

pub(super) fn select_host_call(
    native_plan: &NativePlan,
    host_call: &HostCall,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall {
            platform_call: host_call.platform_call.clone(),
        },
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });

    if let Some(read_line) = runtime_text_line_read(native_plan, host_call) {
        selected_instructions.push(SelectedInstruction {
            kind: read_line,
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
        return;
    }

    let Some(operations) = native_plan.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for operation in operations {
        let operation_operands = select_host_operation_operands(
            native_plan,
            host_call,
            &operation.capability,
            &operation.operation,
        );
        let operation_operands = operands.insert_many(operation_operands);

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: operation.capability.clone(),
                operation: operation.operation.clone(),
                operands: operation_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }

    if host_call_appends_newline(host_call)
        && runtime_machine_string_descriptor_offset(native_plan, host_call).is_some()
        && let Some(newline) = newline_data_object(native_plan)
    {
        let newline_operands = operands.insert_many(vec![
            operand(InstructionOperandKind::ImmediateInteger(1)),
            operand(InstructionOperandKind::DataAddress {
                symbol: newline.symbol.clone(),
            }),
            operand(InstructionOperandKind::ByteLength(1)),
        ]);
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                capability: "Stdout".to_owned(),
                operation: "write".to_owned(),
                operands: newline_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }
}

fn newline_data_object(native_plan: &NativePlan) -> Option<&NativeDataObject> {
    native_plan
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
