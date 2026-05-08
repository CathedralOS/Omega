mod runtime_text;

use crate::abi::PlatformCallData;
use crate::data::NativeDataObject;
use crate::host_calls::{HostCall, HostCallArgument, HostCallArgumentKind};
use crate::plan::NativePlan;
use crate::target::ObjectFormat;
use omega_core::arena::Arena;

use super::model::{
    InstructionOperand, InstructionOperandKind, SelectedInstruction, SelectedInstructionKind,
};
use runtime_text::{
    find_runtime_text_input_buffer_data_object, runtime_text_line_read,
    runtime_text_literal_for_host_call,
};
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
        source_machine: host_call.machine.clone(),
        source_state: host_call.state.clone(),
        source_statement: host_call.statement_index,
    });

    if let Some(read_line) = runtime_text_line_read(native_plan, host_call) {
        selected_instructions.push(SelectedInstruction {
            kind: read_line,
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
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
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
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
            source_machine: host_call.machine.clone(),
            source_state: host_call.state.clone(),
            source_statement: host_call.statement_index,
        });
    }
}

fn select_host_operation_operands(
    native_plan: &NativePlan,
    host_call: &HostCall,
    capability: &str,
    operation: &str,
) -> Vec<InstructionOperand> {
    match (native_plan.target.object_format, capability, operation) {
        (ObjectFormat::Coff, "Stdout", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-11))]
        }
        (ObjectFormat::Coff, "Stdin", "get_std_handle") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(-10))]
        }
        (_, "Stdin", "read" | "read_file") => {
            let Some(data_object) = find_data_object(native_plan, host_call) else {
                return Vec::new();
            };
            let byte_count = native_plan
                .data
                .bytes
                .span(data_object.bytes)
                .map_or(0, |bytes| bytes.len());

            let mut operands = Vec::new();
            if operation == "read" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(0)));
            }
            operands.push(operand(InstructionOperandKind::DataAddress {
                symbol: data_object.symbol.clone(),
            }));
            operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
            operands
        }
        (_, "Stdout", "write" | "write_file") => {
            let mut operands = Vec::new();
            if operation == "write" {
                operands.push(operand(InstructionOperandKind::ImmediateInteger(1)));
            }

            if let Some(data_object) = find_data_object(native_plan, host_call) {
                let byte_count = native_plan
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .map_or(0, |bytes| bytes.len());
                operands.push(operand(InstructionOperandKind::DataAddress {
                    symbol: data_object.symbol.clone(),
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(byte_count)));
                return operands;
            }

            if let Some(data_object) =
                find_runtime_text_input_buffer_data_object(native_plan, host_call)
                && let Some(literal) = runtime_text_literal_for_host_call(native_plan, host_call)
                && runtime_machine_string_descriptor_offset(native_plan, host_call).is_none()
            {
                operands.push(operand(InstructionOperandKind::DataAddress {
                    symbol: data_object.symbol.clone(),
                }));
                operands.push(operand(InstructionOperandKind::ByteLength(literal.len())));
                return operands;
            }

            if let Some(byte_offset) =
                runtime_machine_string_descriptor_offset(native_plan, host_call)
            {
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
        (_, "Process", "exit" | "exit_group" | "exit_process") => {
            vec![operand(InstructionOperandKind::ImmediateInteger(
                exit_code(host_call, native_plan),
            ))]
        }
        _ => Vec::new(),
    }
}

fn find_data_object<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == host_call.source_key
                && data_object.source_statement == host_call.statement_index
        })
        .map(|(_, data_object)| data_object)
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

fn exit_code(host_call: &HostCall, native_plan: &NativePlan) -> i64 {
    first_argument(host_call, native_plan)
        .and_then(|argument| match &argument.kind {
            HostCallArgumentKind::Integer(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(0)
}

fn first_argument<'plan>(
    host_call: &HostCall,
    native_plan: &'plan NativePlan,
) -> Option<&'plan HostCallArgument> {
    native_plan
        .host_calls
        .arguments
        .span(host_call.arguments)
        .and_then(|arguments| arguments.first())
}

fn operand(kind: InstructionOperandKind) -> InstructionOperand {
    InstructionOperand { kind }
}
