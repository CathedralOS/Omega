use crate::EmissionPlanningInput;
use omega_calling_conventions::{HostCapability, HostOperation, PlatformCallData};
use omega_control_flow::StateKey;
use omega_platform_interface::{HostCall, HostCallArgumentKind};
use omega_runtime_text::places::expression_place_eq_in_table;
use omega_runtime_text::{RuntimeTextSource, RuntimeTextUse};
use omega_state_schedule::{ScheduledState, scheduled_state_contains_key};
use omega_target_operations::{
    InstructionOperandKind, SelectedInstructionKind, TargetDataObjectHandle, TargetDataObjectKind,
};
use psi_arena::Arena;

use super::selected_instruction_queries::{host_operation, host_operation_operands};
use super::semantic_scope::{proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

pub(super) fn collect_host_argument_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    collect_console_byte_op_blockers(input, state_schedule, blockers);
    collect_computed_scalar_argument_blockers(input, state_schedule, blockers);

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

fn collect_computed_scalar_argument_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in input.host_calls.calls.iter() {
        if !scheduled_state_contains_key(state_schedule, host_call.source_key) {
            continue;
        }
        let Some(arguments) = input.host_calls.arguments.span(host_call.arguments) else {
            continue;
        };
        for (index, argument) in arguments.iter().enumerate() {
            let HostCallArgumentKind::Expression(expression) = argument.kind else {
                continue;
            };
            let is_computed_scalar = match input.host_calls.expressions.expression(expression) {
                psi_checked_trees::expression::ExpressionNode::Binary(_)
                | psi_checked_trees::expression::ExpressionNode::Cast(_) => true,
                psi_checked_trees::expression::ExpressionNode::Indexed(indexed) => !matches!(
                    input.host_calls.expressions.expression(indexed.index),
                    psi_checked_trees::expression::ExpressionNode::Range(_)
                ),
                // Selection admits only the scalar builtin symbols. An
                // authored nested call reaches this blocker too and stays
                // fail-closed until explicit call sequencing exists.
                psi_checked_trees::expression::ExpressionNode::Call(_) => true,
                _ => false,
            };
            if !is_computed_scalar {
                continue;
            }
            let target_offset = input.runtime_storage.host_argument_scratch_base + index * 8;
            let has_materialization = input.instructions.code.instructions.iter().any(
                |(_, instruction)| {
                    state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                        && instruction.source_statement == host_call.statement_index
                        && (matches!(
                            instruction.kind,
                            SelectedInstructionKind::WritePlaceBinary { target, .. }
                                if target.region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                    && target.const_offset() == Some(target_offset)
                        ) || matches!(
                            instruction.kind,
                            SelectedInstructionKind::WriteRuntimeStorageConvert {
                                target_region,
                                target_offset: actual_offset,
                                ..
                            } if target_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                && actual_offset == target_offset
                        ) || matches!(
                            instruction.kind,
                            SelectedInstructionKind::CopyPlaces { target, .. }
                                if target.region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                    && target.const_offset() == Some(target_offset)
                        ))
                },
            );
            if has_materialization {
                continue;
            }
            if matches!(
                input.host_calls.expressions.expression(expression),
                psi_checked_trees::expression::ExpressionNode::Call(_)
            ) && host_call_argument_has_selected_call_result_operand(
                input, host_call, expression,
            ) {
                continue;
            }
            let source_name = state_name(input, host_call.source_key);
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{} statement {} computed scalar argument {} did not materialize before its platform call and is not encodable{}",
                    source_name,
                    host_call.statement_index,
                    index,
                    proof_scope_suffix(input, host_call.source_key)
                ),
            ));
        }
    }
}

fn host_call_argument_has_selected_call_result_operand(
    input: &EmissionPlanningInput<'_>,
    host_call: &HostCall,
    expression: psi_checked_trees::expression::ExpressionHandle,
) -> bool {
    let psi_checked_trees::expression::ExpressionNode::Call(call) =
        input.host_calls.expressions.expression(expression)
    else {
        return false;
    };
    let result_slots = input
        .state_calls
        .calls_for_statement(host_call.source_key, host_call.statement_index)
        .filter(|state_call| {
            if state_call.role != omega_state_calls::StateCallRole::CallArgument {
                return false;
            }
            let target_name = input
                .control_flow
                .state_by_key(state_call.target_key)
                .map(|state| state.name.as_str());
            state_call.target_key.state == call.target_symbol
                || target_name == Some(call.target.as_str())
        })
        .flat_map(|state_call| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(move |(_, slot)| {
                    (state_key_matches_statement_source(slot.source_key, state_call.source_key)
                        && slot.statement_index == state_call.statement_index
                        && matches!(
                            slot.kind,
                            omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult {
                                role: omega_state_calls::StateCallRole::CallArgument,
                                call_ordinal,
                                ..
                            } if call_ordinal == state_call.call_ordinal
                        ))
                    .then_some((slot.byte_offset, slot.byte_size))
                })
        })
        .collect::<Vec<_>>();
    if result_slots.is_empty() {
        return false;
    }

    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            if !state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                || instruction.source_statement != host_call.statement_index
            {
                return false;
            }
            let Some(operands) = host_operation_operands(&instruction.kind) else {
                return false;
            };
            input
                .instructions
                .code
                .operands
                .span(operands)
                .is_some_and(|operands| {
                    operands.iter().any(|operand| {
                        let (region, byte_offset, byte_count) = match operand.kind {
                            InstructionOperandKind::RuntimeScalarInteger {
                                region,
                                byte_offset,
                                byte_count,
                            }
                            | InstructionOperandKind::RuntimeScalarFloat {
                                region,
                                byte_offset,
                                byte_count,
                            } => (region, byte_offset, byte_count),
                            _ => return false,
                        };
                        region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            && result_slots.contains(&(byte_offset, byte_count))
                    })
                })
        })
}

/// Console byte ops select into their composite instructions ONLY (selection
/// emits nothing generic for them), so a resolution miss would otherwise be
/// a SILENTLY DROPPED call -- the serve-or-refuse cardinal sin. Any
/// SingleByteRead/SingleByteWrite host call in a scheduled state must have
/// its composite; a miss blocks the compile loudly.
fn collect_console_byte_op_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in input.host_calls.calls.iter() {
        if !scheduled_state_contains_key(state_schedule, host_call.source_key) {
            continue;
        }
        let (op_name, needs_read) = match host_call.data {
            PlatformCallData::SingleByteRead => ("read_byte", true),
            PlatformCallData::SingleByteWrite => ("write_byte", false),
            _ => continue,
        };

        let has_composite = input
            .instructions
            .code
            .instructions
            .iter()
            .any(|(_, instruction)| {
                state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                    && instruction.source_statement == host_call.statement_index
                    && if needs_read {
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::ReadRuntimeByte { .. }
                        )
                    } else {
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::WriteRuntimeByte { .. }
                        )
                    }
            });
        if has_composite {
            continue;
        }

        let source_name = state_name(input, host_call.source_key);
        blockers.insert(blocker(
            "host arguments",
            &format!(
                "{} statement {} console `{op_name}` did not lower to its byte-op instruction \
                 (read_byte serves the `let r: ByteRead = ...` local shape; write_byte serves \
                 place-backed or integer-literal arguments){}",
                source_name,
                host_call.statement_index,
                proof_scope_suffix(input, host_call.source_key)
            ),
        ));
    }
}

fn collect_selected_runtime_text_buffer_host_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, instruction) in input.instructions.code.instructions.iter() {
        let Some(operands) = host_operation_operands(&instruction.kind) else {
            continue;
        };
        let Some(operands) = input.instructions.code.operands.span(operands) else {
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
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                && instruction.source_statement == host_call.statement_index
                && host_operation(&instruction.kind).is_some()
        })
}

fn host_text_argument_has_planned_text_operands(
    input: &EmissionPlanningInput<'_>,
    host_call: &HostCall,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            if !state_key_matches_statement_source(instruction.source_key, host_call.source_key)
                || instruction.source_statement != host_call.statement_index
            {
                return false;
            }

            let Some(operands) = host_operation_operands(&instruction.kind) else {
                return false;
            };
            let Some(operands) = input.instructions.code.operands.span(operands) else {
                return false;
            };

            let has_runtime_pointer = operands.iter().any(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::RuntimeStringPointer { .. }
                        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                )
            });
            let has_runtime_length = operands.iter().any(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::RuntimeStringLength { .. }
                        | InstructionOperandKind::RuntimePointeeStringLength { .. }
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
        .code
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
        | SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, .. }
        | SelectedInstructionKind::AppendTextStoredToPlace { buffer, .. }
        | SelectedInstructionKind::AppendTextLiteralToPlace { buffer, .. }
        | SelectedInstructionKind::ReadRuntimeTextLine { buffer, .. } => *buffer == data_handle,

        _ => host_operation(kind)
            .filter(|(operation_key, _)| {
                operation_key.capability == HostCapability::Stdin
                    && operation_key.operation == HostOperation::ReadFile
            })
            .and_then(|(_, operands)| input.instructions.code.operands.span(operands))
            .is_some_and(|operands| {
                operands.iter().any(|operand| {
                    let InstructionOperandKind::DataAddress { data } = operand.kind else {
                        return false;
                    };
                    matches!(
                        data,
                        remapped if remapped == data_handle
                    )
                })
            }),
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
