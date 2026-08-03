use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_runtime_text::places::expression_place_eq_in_table;
use omega_runtime_text::{RuntimeTextWrite, RuntimeTextWriteKind};
use omega_state_storage::StateMutationLowering;
use omega_state_values::{StateValueKind, StateValueRole, StateValueUse};
use omega_target_operations::SelectedInstructionKind;
use psi_arena::Arena;

use super::semantic_scope::{proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_value_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, value) in input.state_values.values.iter() {
        if !value.required || value.kind != StateValueKind::Binary {
            continue;
        }

        if !runtime_body_has_statement(input, value.source_key, value.statement_index) {
            continue;
        }

        if state_value_is_static_assignment(input, value) {
            continue;
        }

        if state_value_has_planned_storage_write(input, value) {
            continue;
        }

        if state_value_has_planned_text_builder(input, value) {
            continue;
        }

        blockers.insert(blocker(
            "state values",
            &runtime_value_blocker_reason(input, value),
        ));
    }
}

fn runtime_body_has_statement(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .runtime_bodies
        .operations
        .iter()
        .any(|(_, operation)| {
            state_key_matches_statement_source(operation.source_key, source_key)
                && operation.statement_index == statement_index
        })
}

fn state_value_has_planned_text_builder(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> bool {
    runtime_text_write_for_statement(input, value.source_key, value.statement_index)
        .is_some_and(|text_write| runtime_text_write_is_planned(input, text_write))
}

fn state_value_has_planned_storage_write(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> bool {
    if value.role == StateValueRole::TransitionArgument
        && runtime_transition_arguments_are_materialized(
            input,
            value.source_key,
            value.statement_index,
        )
    {
        return true;
    }

    // A binary call argument (e.g. `self.carve(.., 100 + index)`) is not
    // materialized into a frame slot at the call site -- the call is inlined and
    // the argument expression is folded into the callee body's writes (under the
    // callee's source key). When the call resolves to a target state, the inlined
    // body is responsible for lowering the argument value, so the call-site value
    // is materialized.
    if value.role == StateValueRole::CallArgument
        && runtime_call_argument_is_inlined(input, value.source_key, value.statement_index)
    {
        return true;
    }

    if !matches!(
        value.role,
        StateValueRole::AssignmentValue
            | StateValueRole::TransitionArgument
            | StateValueRole::CallArgument
    ) {
        return false;
    }

    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches_statement_source(instruction.source_key, value.source_key)
                && instruction.source_statement == value.statement_index
                && matches!(
                    instruction.kind,
                    SelectedInstructionKind::AtomicLoad { .. }
                        | SelectedInstructionKind::AtomicStore { .. }
                        | SelectedInstructionKind::AtomicFetchAdd { .. }
                        | SelectedInstructionKind::AtomicFetchSub { .. }
                        | SelectedInstructionKind::AtomicFetchXor { .. }
                        | SelectedInstructionKind::AtomicFetchOr { .. }
                        | SelectedInstructionKind::AtomicFetchAnd { .. }
                        | SelectedInstructionKind::AtomicSwap { .. }
                        | SelectedInstructionKind::HostOperation { .. }
                        // The console byte-op composites consume a RESOLVED
                        // place (selection refuses otherwise): a bound local
                        // argument's value was already lowered at its own
                        // LocalData statement, exactly like the HostOperation
                        // arm above -- without these the blocker re-fired on
                        // the bind-first shape it itself instructs
                        // (write_byte(rotated) after `let rotated = b + 1`).
                        | SelectedInstructionKind::ReadRuntimeByte { .. }
                        | SelectedInstructionKind::WriteRuntimeByte { .. }
                        | SelectedInstructionKind::AtomicCompareExchange { .. }
                        | SelectedInstructionKind::WritePlaceInteger { .. }
                        | SelectedInstructionKind::WriteStorageBitField { .. }
                        | SelectedInstructionKind::WritePlaceBinary { .. }
                        | SelectedInstructionKind::WritePlaceString { .. }
                        | SelectedInstructionKind::WritePlaceBoundedBuffer { .. }
                        | SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { .. }
                        | SelectedInstructionKind::AppendPlaceBoundedBufferSource { .. }
                        | SelectedInstructionKind::WritePlaceAddress { .. }
                        | SelectedInstructionKind::WriteRuntimeStorageConvert { .. }
                        | SelectedInstructionKind::WritePlaceConvert { .. }
                        | SelectedInstructionKind::CopyPlaces { .. }
                        // `asm { in <local>, <port> }`: the PortRead writes the
                        // byte into the local's place, covering the assignment
                        // value (a local-dest port read; a field dest never
                        // creates a runtime-value record).
                        | SelectedInstructionKind::PortRead { .. }
                        | SelectedInstructionKind::FlagsSnapshot { .. }
                        | SelectedInstructionKind::MsrRead { .. }
                        | SelectedInstructionKind::ControlRegisterRead { .. }
                )
        })
}

fn runtime_call_argument_is_inlined(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.required
            && state_call.target_key.is_valid()
            && state_call.statement_index == statement_index
            && state_key_matches_statement_source(state_call.source_key, source_key)
    })
}

fn runtime_transition_arguments_are_materialized(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    if input.runtime_flow.edges.iter().any(|(_, edge)| {
        edge.from == source_key
            && edge.statement_index == statement_index
            && (!edge.expressions.target_arguments.is_empty()
                || !edge.expressions.continuation_arguments.is_empty())
    }) {
        return true;
    }

    input.runtime_dispatch_loop.cases.iter().any(|(_, case)| {
        case.key == source_key
            && input
                .runtime_dispatch_loop
                .edges
                .span(case.edges)
                .unwrap_or(&[])
                .iter()
                .any(|edge| {
                    edge.statement_index == statement_index
                        && (!edge.target_arguments.is_empty()
                            || !edge.continuation_arguments.is_empty())
                })
    })
}

fn runtime_value_blocker_reason(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> String {
    if let Some(text_write) =
        runtime_text_write_for_statement(input, value.source_key, value.statement_index)
    {
        let source_name = state_name(input, text_write.source_key);
        return format!(
            "{} statement {} text write `{}` = `{}`{} needs {}",
            source_name,
            text_write.statement_index,
            input
                .runtime_text
                .expressions
                .display_name(text_write.target),
            input
                .runtime_text
                .expressions
                .display_name(text_write.value),
            proof_scope_suffix(input, text_write.source_key),
            runtime_text_write_lowering_name(text_write)
        );
    }

    let source_name = state_name(input, value.source_key);
    format!(
        "{} statement {} {:?} binary expression `{}`{} needs runtime value lowering",
        source_name,
        value.statement_index,
        value.role,
        input
            .state_values
            .expressions
            .display_name(value.expression),
        proof_scope_suffix(input, value.source_key)
    )
}

pub(super) fn runtime_text_write_for_statement<'plan>(
    input: &'plan EmissionPlanningInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan RuntimeTextWrite> {
    input
        .runtime_text
        .writes
        .iter()
        .find(|(_, text_write)| {
            text_write.source_key == source_key && text_write.statement_index == statement_index
        })
        .map(|(_, text_write)| text_write)
}

fn runtime_text_builder_for_write<'plan>(
    input: &'plan EmissionPlanningInput<'plan>,
    text_write: &RuntimeTextWrite,
) -> Option<&'plan omega_runtime_text::RuntimeTextBuilder> {
    input
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == text_write.source_key
                && builder.statement_index == text_write.statement_index
                && expression_place_eq_in_table(
                    &input.runtime_text.expressions,
                    builder.target,
                    text_write.target,
                )
        })
        .map(|(_, builder)| builder)
}

fn runtime_text_write_lowering_name(text_write: &RuntimeTextWrite) -> &'static str {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText => "runtime text literal storage",
        RuntimeTextWriteKind::StoredCopy => "runtime text copy lowering",
        RuntimeTextWriteKind::GeneratedString => "runtime string builder lowering",
        RuntimeTextWriteKind::OtherExpression => "runtime text expression lowering",
    }
}

fn state_value_is_static_assignment(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> bool {
    if !matches!(
        value.role,
        StateValueRole::AssignmentValue | StateValueRole::TransitionArgument
    ) {
        return false;
    }

    input.state_storage.mutations.iter().any(|(_, mutation)| {
        mutation.source_key == value.source_key
            && mutation.statement_index == value.statement_index
            && mutation.lowering == StateMutationLowering::AlreadyLowered
    })
}

pub(super) fn runtime_text_write_is_planned(
    input: &EmissionPlanningInput<'_>,
    text_write: &RuntimeTextWrite,
) -> bool {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText | RuntimeTextWriteKind::StoredCopy => {
            runtime_text_write_has_selected_instruction(input, text_write)
        }
        RuntimeTextWriteKind::GeneratedString => {
            runtime_text_builder_for_write(input, text_write).is_some()
                && runtime_text_write_has_selected_instruction(input, text_write)
        }
        RuntimeTextWriteKind::OtherExpression => false,
    }
}

fn runtime_text_write_has_selected_instruction(
    input: &EmissionPlanningInput<'_>,
    text_write: &RuntimeTextWrite,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches_statement_source(instruction.source_key, text_write.source_key)
                && instruction.source_statement == text_write.statement_index
                && matches!(
                    instruction.kind,
                    SelectedInstructionKind::WritePlaceString { .. }
                        | SelectedInstructionKind::WritePlaceBoundedBuffer { .. }
                        | SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { .. }
                        | SelectedInstructionKind::AppendPlaceBoundedBufferSource { .. }
                        | SelectedInstructionKind::WritePlaceAddress { .. }
                        | SelectedInstructionKind::WriteRuntimeTextLiteral { .. }
                        | SelectedInstructionKind::WriteRuntimeTextLiteralSegment { .. }
                        | SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. }
                        | SelectedInstructionKind::MaterializeTextBufferToPlace { .. }
                        | SelectedInstructionKind::AppendTextStoredToPlace { .. }
                        | SelectedInstructionKind::AppendTextLiteralToPlace { .. }
                        | SelectedInstructionKind::CopyPlaces { .. }
                        // `asm { in <local>, <port> }`: the PortRead writes the
                        // byte into the local's place, covering the assignment
                        // value (a local-dest port read; a field dest never
                        // creates a runtime-value record).
                        | SelectedInstructionKind::PortRead { .. }
                        | SelectedInstructionKind::FlagsSnapshot { .. }
                        | SelectedInstructionKind::MsrRead { .. }
                        | SelectedInstructionKind::ControlRegisterRead { .. }
                )
        })
}

fn state_key_matches_statement_source(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}
