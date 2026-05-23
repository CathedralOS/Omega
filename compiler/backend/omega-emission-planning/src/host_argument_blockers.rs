use crate::EmissionPlanningInput;
use omega_calling_conventions::{HostCapability, HostOperation, PlatformCallData};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_platform_interface::{HostCall, HostCallArgumentKind};
use omega_runtime_text::places::expression_place_eq_in_table;
use omega_runtime_text::{RuntimeTextSource, RuntimeTextUse};
use omega_state_schedule::{ScheduledState, scheduled_state_contains_key};
use omega_target_operations::{
    InstructionOperandKind, SelectedInstructionKind, TargetDataObjectHandle, TargetDataObjectKind,
};

use super::semantic_scope::{proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

pub(super) fn collect_host_argument_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in input.host_calls.calls.iter() {
        if !scheduled_state_contains_key(state_schedule, host_call.source_key) {
            continue;
        }

        let PlatformCallData::FirstTextArgument { .. } = host_call.data else {
            continue;
        };
        let Some(arguments) = input.host_calls.arguments.span(host_call.arguments) else {
            let source_name = state_name(input, host_call.source_key);
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{} statement {} has an invalid argument span",
                    source_name, host_call.statement_index
                ),
            ));
            continue;
        };
        let Some(first_argument) = arguments.first() else {
            let source_name = state_name(input, host_call.source_key);
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{} statement {} needs a text argument",
                    source_name, host_call.statement_index
                ),
            ));
            continue;
        };

        if let HostCallArgumentKind::Expression(expression) = &first_argument.kind {
            if !host_call_has_selected_operation(input, host_call) {
                continue;
            }

            let runtime_text_use = runtime_text_use_for_host_call(input, host_call);
            if runtime_text_use.is_some_and(|text_use| {
                runtime_text_use_has_materialized_input_buffer(input, text_use)
            }) || host_text_argument_has_planned_text_operands(input, host_call)
            {
                continue;
            }
            blockers.insert(blocker(
                "host arguments",
                &runtime_text_use
                    .map(|text_use| host_text_argument_blocker_reason(input, text_use))
                    .unwrap_or_else(|| {
                        let source_name = state_name(input, host_call.source_key);
                        format!(
                            "{} statement {} text argument `{}`{} needs runtime text lowering",
                            source_name,
                            host_call.statement_index,
                            input.host_calls.expressions.display_name(*expression),
                            proof_scope_suffix(input, host_call.source_key)
                        )
                    }),
            ));
        }
    }

    collect_selected_runtime_text_buffer_host_blockers(input, blockers);
}

fn collect_selected_runtime_text_buffer_host_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, instruction) in input.instructions.instructions.iter() {
        let SelectedInstructionKind::HostOperation { operands, .. } = instruction.kind else {
            continue;
        };
        let Some(operands) = input.instructions.operands.span(operands) else {
            continue;
        };

        for operand in operands {
            let InstructionOperandKind::DataAddress { data } = operand.kind else {
                continue;
            };
            if !data.is_valid() {
                continue;
            }
            let data_object = input.data.objects.get(data);
            if data_object.kind != TargetDataObjectKind::RuntimeTextBuffer {
                continue;
            }
            if runtime_text_buffer_has_selected_producer(input, data) {
                continue;
            }

            let source_name = state_name(input, instruction.source_key);
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{} statement {} host text buffer `{}` needs runtime text materialization{}",
                    source_name,
                    instruction.source_statement,
                    data_object.symbol,
                    proof_scope_suffix(input, instruction.source_key)
                ),
            ));
        }
    }
}

fn host_call_has_selected_operation(
    input: &EmissionPlanningInput<'_>,
    host_call: &HostCall,
) -> bool {
    input
        .instructions
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                && instruction.source_statement == host_call.statement_index
                && matches!(
                    instruction.kind,
                    SelectedInstructionKind::HostOperation { .. }
                )
        })
}

fn host_text_argument_has_planned_text_operands(
    input: &EmissionPlanningInput<'_>,
    host_call: &HostCall,
) -> bool {
    input
        .instructions
        .instructions
        .iter()
        .any(|(_, instruction)| {
            if !state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                || instruction.source_statement != host_call.statement_index
            {
                return false;
            }

            let SelectedInstructionKind::HostOperation { operands, .. } = instruction.kind else {
                return false;
            };
            let Some(operands) = input.instructions.operands.span(operands) else {
                return false;
            };

            let has_runtime_pointer = operands.iter().any(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::RuntimeStringPointer { .. }
                )
            });
            let has_runtime_length = operands.iter().any(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::RuntimeStringLength { .. }
                )
            });
            let has_materialized_data_address = operands.iter().any(|operand| {
                let InstructionOperandKind::DataAddress { data } = operand.kind else {
                    return false;
                };
                let data_object = input.data.objects.get(data);
                if data_object.kind != TargetDataObjectKind::RuntimeTextBuffer {
                    return true;
                }

                runtime_text_buffer_has_selected_producer(input, data)
            });
            let has_byte_length = operands
                .iter()
                .any(|operand| matches!(operand.kind, InstructionOperandKind::ByteLength(_)));

            (has_runtime_pointer && has_runtime_length)
                || (has_materialized_data_address && has_byte_length)
        })
}

fn state_key_matches_statement_source(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}

fn runtime_text_use_for_host_call<'plan>(
    input: &'plan EmissionPlanningInput<'plan>,
    host_call: &HostCall,
) -> Option<&'plan RuntimeTextUse> {
    input
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            state_key_matches_statement_source(text_use.source_key, host_call.source_key)
                && text_use.statement_index == host_call.statement_index
        })
        .map(|(_, text_use)| text_use)
}

fn runtime_text_use_has_materialized_input_buffer(
    input: &EmissionPlanningInput<'_>,
    text_use: &RuntimeTextUse,
) -> bool {
    if let Some(buffer) = input.runtime_text.buffers.iter().find_map(|(_, buffer)| {
        (state_key_matches_statement_source(buffer.source_key, text_use.source_key)
            && buffer.statement_index == text_use.statement_index
            && text_use.source == RuntimeTextSource::StoredPlace)
            .then_some(buffer)
    }) {
        let data_handle = runtime_text_buffer_data_handle(input, buffer);
        return data_handle.is_valid()
            && runtime_text_buffer_has_selected_producer(input, data_handle);
    }

    let Some(slot) = input.runtime_text.slots.iter().find_map(|(_, slot)| {
        expression_place_eq_in_table(
            &input.runtime_text.expressions,
            slot.place,
            text_use.expression,
        )
        .then_some(slot)
    }) else {
        return false;
    };

    if !slot.has_input_buffer {
        return false;
    }

    input
        .runtime_text
        .buffers
        .iter()
        .filter(|(_, buffer)| {
            expression_place_eq_in_table(
                &input.runtime_text.expressions,
                buffer.text_place,
                slot.place,
            )
        })
        .map(|(_, buffer)| runtime_text_buffer_data_handle(input, buffer))
        .any(|data_handle| {
            data_handle.is_valid() && runtime_text_buffer_has_selected_producer(input, data_handle)
        })
}

fn runtime_text_buffer_data_handle(
    input: &EmissionPlanningInput<'_>,
    buffer: &omega_runtime_text::RuntimeTextBuffer,
) -> TargetDataObjectHandle {
    input
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == buffer.source_key
                && data_object.source_statement == buffer.statement_index
        })
        .map(|(handle, _)| handle)
        .unwrap_or_else(TargetDataObjectHandle::invalid)
}

fn runtime_text_buffer_has_selected_producer(
    input: &EmissionPlanningInput<'_>,
    data_handle: TargetDataObjectHandle,
) -> bool {
    input
        .instructions
        .instructions
        .iter()
        .any(|(_, instruction)| {
            selected_instruction_writes_runtime_text_buffer(input, &instruction.kind, data_handle)
        })
}

fn selected_instruction_writes_runtime_text_buffer(
    input: &EmissionPlanningInput<'_>,
    kind: &SelectedInstructionKind,
    data_handle: TargetDataObjectHandle,
) -> bool {
    match kind {
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. }
        | SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. }
        | SelectedInstructionKind::AppendRuntimeTextStoredSuffix { buffer, .. }
        | SelectedInstructionKind::MaterializeRuntimeTextBuffer { buffer, .. }
        | SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            buffer, ..
        }
        | SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            buffer,
            ..
        }
        | SelectedInstructionKind::AppendRuntimeTextStoredPlace { buffer, .. }
        | SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            buffer, ..
        }
        | SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            buffer,
            ..
        }
        | SelectedInstructionKind::AppendRuntimeTextLiteral { buffer, .. }
        | SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee { buffer, .. }
        | SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            buffer, ..
        } => *buffer == data_handle,
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } if operation_key.capability == HostCapability::Stdin
            && operation_key.operation == HostOperation::ReadFile =>
        {
            input
                .instructions
                .operands
                .span(*operands)
                .unwrap_or(&[])
                .iter()
                .any(|operand| {
                    matches!(
                        operand.kind,
                        InstructionOperandKind::DataAddress { data } if data == data_handle
                    )
                })
        }
        _ => false,
    }
}

fn host_text_argument_blocker_reason(
    input: &EmissionPlanningInput<'_>,
    text_use: &RuntimeTextUse,
) -> String {
    let lowering_need = match text_use.source {
        RuntimeTextSource::StoredPlace => "runtime string storage lowering",
        RuntimeTextSource::GeneratedString => "runtime string builder lowering",
        RuntimeTextSource::MutablePlace => "runtime mutable string place lowering",
        RuntimeTextSource::OtherExpression => "runtime string expression lowering",
    };

    let source_name = state_name(input, text_use.source_key);
    format!(
        "{} statement {} text argument `{}`{} needs {lowering_need}",
        source_name,
        text_use.statement_index,
        input
            .runtime_text
            .expressions
            .display_name(text_use.expression),
        proof_scope_suffix(input, text_use.source_key)
    )
}
