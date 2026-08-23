mod operands;
mod runtime_text;

use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasResolutionContext;
use omega_abstract_operations::{AbstractDataObject, AbstractDataObjectKind};
use omega_calling_conventions::{
    HostCapability, HostOperation, HostOperationKey, PlatformCallData,
};
use omega_platform_interface::HostCall;
use psi_arena::Arena;

use super::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    InstructionOperand, InstructionOperandKind, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind,
};
pub(in crate::selection) use operands::system_v_record_descriptor_shape;
use operands::{data_object_handle, operand, select_host_operation_operands};
use runtime_text::runtime_text_line_read;
pub(super) use runtime_text::{
    runtime_string_descriptor_place, runtime_text_input_buffer_data_for_text_place,
    runtime_text_input_buffer_data_for_text_place_in_table,
    runtime_text_literal_write_for_host_call,
};

pub(super) fn select_host_call(
    input: &InstructionSelectionInput<'_>,
    host_call: &HostCall,
    dispatch_index: Option<u32>,
    alias_context: Option<RuntimeAliasResolutionContext<'_, '_>>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if input.runtime_storage.host_argument_scratch_size > 0
        && let Some(arguments) = input.host_calls.arguments.span(host_call.arguments)
    {
        for (index, argument) in arguments.iter().enumerate() {
            let omega_platform_interface::HostCallArgumentKind::Expression(expression) =
                argument.kind
            else {
                continue;
            };
            let target_offset = input.runtime_storage.host_argument_scratch_base + index * 8;
            if let Some((kind, _)) =
                crate::selection::runtime_dispatch::select_computed_host_argument_write(
                    input,
                    dispatch_index.unwrap_or(0),
                    host_call.source_key,
                    host_call.statement_index,
                    &input.host_calls.expressions,
                    expression,
                    target_offset,
                    runtime_value_operands,
                )
            {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key: host_call.source_key,
                    source_statement: host_call.statement_index,
                });
            }
        }
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::BeginPlatformCall,
        source_key: host_call.source_key,
        source_statement: host_call.statement_index,
    });

    if let Some(read_line) = runtime_text_line_read(input, host_call, dispatch_index, alias_context)
    {
        selected_instructions.push(SelectedInstruction {
            kind: read_line,
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
        return;
    }

    // Console byte ops select into their composite instructions ONLY -- on a
    // resolution miss nothing generic is emitted (the byte-op blocker in
    // emission planning turns the miss into a loud compile refusal rather
    // than a mismatched generic call).
    if matches!(
        host_call.data,
        PlatformCallData::SingleByteRead | PlatformCallData::SingleByteWrite
    ) {
        let selected = runtime_text::runtime_byte_read(input, host_call, dispatch_index)
            .or_else(|| runtime_text::runtime_byte_write(input, host_call, dispatch_index));
        if let Some(byte_op) = selected {
            selected_instructions.push(SelectedInstruction {
                kind: byte_op,
                source_key: host_call.source_key,
                source_statement: host_call.statement_index,
            });
        }
        return;
    }

    let Some(operations) = input.host_calls.operations.span(host_call.operations) else {
        return;
    };

    for (operation_ordinal, operation) in operations.iter().enumerate() {
        let operation_operands = select_host_operation_operands(
            input,
            host_call,
            dispatch_index,
            alias_context,
            operation,
            operands,
        );

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::HostOperation {
                operation_ordinal: operation_ordinal as u16,
                operands: operation_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }

    if host_call_appends_newline(host_call)
        && runtime_string_descriptor_place(input, host_call, dispatch_index, alias_context)
            .is_some()
        && let Some(newline) = newline_data_object(input)
    {
        // The newline is a second WriteFile to the same destination handle as the
        // call body. Recover that destination capability (Stdout vs Stderr) from
        // the lowering's own WriteFile operation so the newline targets the same
        // stream rather than always defaulting to stdout.
        let write_capability = operations
            .iter()
            .find(|operation| {
                matches!(
                    operation.operation_key.operation,
                    HostOperation::Write | HostOperation::WriteFile
                )
            })
            .map(|operation| operation.operation_key.capability)
            .unwrap_or(HostCapability::Stdout);
        let uses_write_file = operations.iter().any(|operation| {
            operation.operation_key
                == HostOperationKey::new(write_capability, HostOperation::WriteFile)
        });
        if uses_write_file
            && let Some(get_std_handle) = operations.iter().find(|operation| {
                operation.operation_key
                    == HostOperationKey::new(write_capability, HostOperation::GetStdHandle)
            })
        {
            let handle_operands = select_host_operation_operands(
                input,
                host_call,
                dispatch_index,
                alias_context,
                get_std_handle,
                operands,
            );
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::PreparePlatformOutputHandle {
                    capability: write_capability,
                    operands: handle_operands,
                },
                source_key: host_call.source_key,
                source_statement: host_call.statement_index,
            });
        }
        let newline_operands = if uses_write_file {
            operands.insert_many([
                operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle(input, newline),
                }),
                operand(InstructionOperandKind::ByteLength(1)),
            ])
        } else {
            operands.insert_many([
                operand(InstructionOperandKind::ImmediateInteger(
                    operands::write_file_descriptor(write_capability),
                )),
                operand(InstructionOperandKind::DataAddress {
                    data: data_object_handle(input, newline),
                }),
                operand(InstructionOperandKind::ByteLength(1)),
            ])
        };
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WritePlatformNewline {
                capability: write_capability,
                use_file_api: uses_write_file,
                operands: newline_operands,
            },
            source_key: host_call.source_key,
            source_statement: host_call.statement_index,
        });
    }
}

fn newline_data_object<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
) -> Option<&'plan AbstractDataObject> {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| data_object.kind == AbstractDataObjectKind::HostNewline)
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
