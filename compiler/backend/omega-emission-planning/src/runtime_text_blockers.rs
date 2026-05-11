use crate::EmissionPlanningInput;
use omega_control_flow::{OperationKind, StateKey};
use omega_core::arena::Arena;
use omega_runtime_text::places::expression_place_eq_in_table;
use omega_runtime_text::{RuntimeTextWrite, RuntimeTextWriteKind};
use omega_state_values::{StateValueKind, StateValueRole, StateValueUse};
use omega_target_operations::SelectedInstructionKind;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_value_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, value) in input.state_values.values.iter() {
        if !value.required || value.kind != StateValueKind::Binary {
            continue;
        }

        if value.role == StateValueRole::TransitionGuard {
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

fn state_value_has_planned_text_builder(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> bool {
    runtime_text_write_for_statement(input, value.source_key, value.statement_index).is_some_and(
        |text_write| {
            text_write.kind == RuntimeTextWriteKind::GeneratedString
                && runtime_text_builder_for_write(input, text_write).is_some()
        },
    )
}

fn state_value_has_planned_storage_write(
    input: &EmissionPlanningInput<'_>,
    value: &StateValueUse,
) -> bool {
    if value.role != StateValueRole::AssignmentValue {
        return false;
    }

    input.instructions.instructions.iter().any(|(_, instruction)| {
        instruction.source_key == value.source_key
            && instruction.source_statement == value.statement_index
            && matches!(
                instruction.kind,
                SelectedInstructionKind::WriteRuntimeMachineInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageBinary { .. }
                    | SelectedInstructionKind::WriteRuntimeFrameIndexedInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { .. }
                    | SelectedInstructionKind::WriteRuntimeMachineString { .. }
                    | SelectedInstructionKind::CopyRuntimeStorage { .. }
                    | SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
            )
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
            "{} statement {} text write `{}` = `{}` needs {}",
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
            runtime_text_write_lowering_name(text_write)
        );
    }

    let source_name = state_name(input, value.source_key);
    format!(
        "{} statement {} {:?} binary expression `{}` needs runtime value lowering",
        source_name,
        value.statement_index,
        value.role,
        input
            .state_values
            .expressions
            .display_name(value.expression)
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
    if value.role != StateValueRole::AssignmentValue {
        return false;
    }
    let Some(state) = input.control_flow.state_by_key(value.source_key) else {
        return false;
    };
    let Some(operations) = input.control_flow.operations.span(state.operations) else {
        return false;
    };

    operations.iter().any(|operation| {
        operation.statement_index == value.statement_index
            && matches!(operation.kind, OperationKind::StaticAssignment { .. })
    })
}

pub(super) fn runtime_text_write_is_planned(
    input: &EmissionPlanningInput<'_>,
    text_write: &RuntimeTextWrite,
) -> bool {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText | RuntimeTextWriteKind::StoredCopy => true,
        RuntimeTextWriteKind::GeneratedString => {
            runtime_text_builder_for_write(input, text_write).is_some()
        }
        RuntimeTextWriteKind::OtherExpression => false,
    }
}

fn state_name(input: &EmissionPlanningInput<'_>, key: StateKey) -> String {
    input
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
