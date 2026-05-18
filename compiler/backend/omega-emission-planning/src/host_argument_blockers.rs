use crate::EmissionPlanningInput;
use omega_calling_conventions::PlatformCallData;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_platform_interface::{HostCall, HostCallArgumentKind};
use omega_runtime_text::places::expression_place_eq_in_table;
use omega_runtime_text::{RuntimeTextSource, RuntimeTextUse};
use omega_state_schedule::{ScheduledState, scheduled_state_contains_key};
use omega_target_operations::{InstructionOperandKind, SelectedInstructionKind};

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
            let runtime_text_use = runtime_text_use_for_host_call(input, host_call);
            if runtime_text_use
                .is_some_and(|text_use| runtime_text_use_has_input_buffer(input, text_use))
                || host_text_argument_has_planned_text_operands(input, host_call)
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
            let has_data_address = operands
                .iter()
                .any(|operand| matches!(operand.kind, InstructionOperandKind::DataAddress { .. }));
            let has_byte_length = operands
                .iter()
                .any(|operand| matches!(operand.kind, InstructionOperandKind::ByteLength(_)));

            (has_runtime_pointer && has_runtime_length) || (has_data_address && has_byte_length)
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

fn runtime_text_use_has_input_buffer(
    input: &EmissionPlanningInput<'_>,
    text_use: &RuntimeTextUse,
) -> bool {
    if input.runtime_text.buffers.iter().any(|(_, buffer)| {
        state_key_matches_statement_source(buffer.source_key, text_use.source_key)
            && buffer.statement_index == text_use.statement_index
            && text_use.source == RuntimeTextSource::StoredPlace
    }) {
        return true;
    }

    input.runtime_text.slots.iter().any(|(_, slot)| {
        expression_place_eq_in_table(
            &input.runtime_text.expressions,
            slot.place,
            text_use.expression,
        ) && slot.has_input_buffer
    })
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
