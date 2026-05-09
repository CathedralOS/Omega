use crate::abi::PlatformCallData;
use crate::host_calls::{HostCall, HostCallArgumentKind};
use crate::plan::NativePlan;
use crate::runtime_text::places::expression_place_eq;
use crate::runtime_text::{RuntimeTextSource, RuntimeTextUse};
use crate::state_schedule::{ScheduledState, scheduled_state_contains_key};
use omega_core::arena::Arena;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_host_argument_blockers(
    native_plan: &NativePlan,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if !scheduled_state_contains_key(state_schedule, host_call.source_key) {
            continue;
        }

        let PlatformCallData::FirstTextArgument { .. } = host_call.data else {
            continue;
        };
        let Some(arguments) = native_plan.host_calls.arguments.span(host_call.arguments) else {
            let source_name = state_name(native_plan, host_call.source_key);
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
            let source_name = state_name(native_plan, host_call.source_key);
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
            let runtime_text_use = runtime_text_use_for_host_call(native_plan, host_call);
            if runtime_text_use
                .is_some_and(|text_use| runtime_text_use_has_input_buffer(native_plan, text_use))
            {
                continue;
            }
            blockers.insert(blocker(
                "host arguments",
                &runtime_text_use
                    .map(|text_use| host_text_argument_blocker_reason(native_plan, text_use))
                    .unwrap_or_else(|| {
                        let source_name = state_name(native_plan, host_call.source_key);
                        format!(
                            "{} statement {} text argument `{}` needs runtime text lowering",
                            source_name,
                            host_call.statement_index,
                            expression.display_name()
                        )
                    }),
            ));
        }
    }
}

fn runtime_text_use_for_host_call<'plan>(
    native_plan: &'plan NativePlan,
    host_call: &HostCall,
) -> Option<&'plan RuntimeTextUse> {
    native_plan
        .runtime_text
        .uses
        .iter()
        .find(|(_, text_use)| {
            text_use.source_key == host_call.source_key
                && text_use.statement_index == host_call.statement_index
                && text_use.platform_call == host_call.platform_call
        })
        .map(|(_, text_use)| text_use)
}

fn runtime_text_use_has_input_buffer(native_plan: &NativePlan, text_use: &RuntimeTextUse) -> bool {
    native_plan.runtime_text.slots.iter().any(|(_, slot)| {
        expression_place_eq(&slot.place, &text_use.expression) && slot.has_input_buffer
    })
}

fn host_text_argument_blocker_reason(
    native_plan: &NativePlan,
    text_use: &RuntimeTextUse,
) -> String {
    let lowering_need = match text_use.source {
        RuntimeTextSource::StoredPlace => "runtime string storage lowering",
        RuntimeTextSource::GeneratedString => "runtime string builder lowering",
        RuntimeTextSource::MutablePlace => "runtime mutable string place lowering",
        RuntimeTextSource::OtherExpression => "runtime string expression lowering",
    };

    let source_name = state_name(native_plan, text_use.source_key);
    format!(
        "{} statement {} text argument `{}` needs {lowering_need}",
        source_name,
        text_use.statement_index,
        text_use.expression.display_name()
    )
}

fn state_name(native_plan: &NativePlan, key: crate::control_flow::StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
