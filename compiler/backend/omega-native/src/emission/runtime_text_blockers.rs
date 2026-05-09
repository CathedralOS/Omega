use crate::plan::NativePlan;
use crate::runtime_text::places::expression_place_eq;
use crate::runtime_text::{RuntimeTextWrite, RuntimeTextWriteKind};
use crate::state_values::{StateValueKind, StateValueRole, StateValueUse};
use omega_control_flow::{OperationKind, StateKey};
use omega_core::arena::Arena;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_value_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, value) in native_plan.state_values.values.iter() {
        if !value.required || value.kind != StateValueKind::Binary {
            continue;
        }

        if value.role == StateValueRole::TransitionGuard {
            continue;
        }

        if state_value_is_static_assignment(native_plan, value) {
            continue;
        }

        if state_value_has_planned_text_builder(native_plan, value) {
            continue;
        }

        blockers.insert(blocker(
            "state values",
            &runtime_value_blocker_reason(native_plan, value),
        ));
    }
}

fn state_value_has_planned_text_builder(native_plan: &NativePlan, value: &StateValueUse) -> bool {
    runtime_text_write_for_statement(native_plan, value.source_key, value.statement_index)
        .is_some_and(|text_write| {
            text_write.kind == RuntimeTextWriteKind::GeneratedString
                && runtime_text_builder_for_write(native_plan, text_write).is_some()
        })
}

fn runtime_value_blocker_reason(native_plan: &NativePlan, value: &StateValueUse) -> String {
    if let Some(text_write) =
        runtime_text_write_for_statement(native_plan, value.source_key, value.statement_index)
    {
        let source_name = state_name(native_plan, text_write.source_key);
        return format!(
            "{} statement {} text write `{}` = `{}` needs {}",
            source_name,
            text_write.statement_index,
            text_write.target.display_name(),
            text_write.value.display_name(),
            runtime_text_write_lowering_name(text_write)
        );
    }

    let source_name = state_name(native_plan, value.source_key);
    format!(
        "{} statement {} {:?} binary expression `{}` needs runtime value lowering",
        source_name,
        value.statement_index,
        value.role,
        value.expression.display_name()
    )
}

pub(super) fn runtime_text_write_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan RuntimeTextWrite> {
    native_plan
        .runtime_text
        .writes
        .iter()
        .find(|(_, text_write)| {
            text_write.source_key == source_key && text_write.statement_index == statement_index
        })
        .map(|(_, text_write)| text_write)
}

fn runtime_text_builder_for_write<'plan>(
    native_plan: &'plan NativePlan,
    text_write: &RuntimeTextWrite,
) -> Option<&'plan crate::runtime_text::RuntimeTextBuilder> {
    native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == text_write.source_key
                && builder.statement_index == text_write.statement_index
                && expression_place_eq(&builder.target, &text_write.target)
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

fn state_value_is_static_assignment(native_plan: &NativePlan, value: &StateValueUse) -> bool {
    if value.role != StateValueRole::AssignmentValue {
        return false;
    }
    let Some(state) = native_plan.control_flow.state_by_key(value.source_key) else {
        return false;
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return false;
    };

    operations.iter().any(|operation| {
        operation.statement_index == value.statement_index
            && matches!(operation.kind, OperationKind::StaticAssignment { .. })
    })
}

pub(super) fn runtime_text_write_is_planned(
    native_plan: &NativePlan,
    text_write: &RuntimeTextWrite,
) -> bool {
    match text_write.kind {
        RuntimeTextWriteKind::StaticText | RuntimeTextWriteKind::StoredCopy => true,
        RuntimeTextWriteKind::GeneratedString => {
            runtime_text_builder_for_write(native_plan, text_write).is_some()
        }
        RuntimeTextWriteKind::OtherExpression => false,
    }
}

fn state_name(native_plan: &NativePlan, key: StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
