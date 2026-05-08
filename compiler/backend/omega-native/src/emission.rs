use crate::abi::PlatformCallData;
use crate::host_calls::{HostCall, HostCallArgumentKind};
use crate::plan::NativePlan;
use crate::runtime_text::{RuntimeTextSource, RuntimeTextUse};
use crate::state_schedule::{build_entry_state_schedule, scheduled_state_contains_key};
use crate::target_output::can_emit_target_output;
use omega_core::arena::Arena;
use omega_target::ObjectFormat;

mod runtime_dispatch_blockers;
mod runtime_text_blockers;
mod state_call_blockers;
mod state_codegen_blockers;
mod state_guard_blockers;
mod storage_blockers;

use runtime_dispatch_blockers::{
    collect_runtime_dispatch_blockers, runtime_and_required_states, runtime_dispatch_loop_blocker,
    runtime_dispatch_loop_can_emit,
};
use runtime_text_blockers::collect_state_value_blockers;
use state_call_blockers::collect_state_call_blockers;
use state_codegen_blockers::collect_state_codegen_blockers;
use state_guard_blockers::collect_state_guard_blockers;
use storage_blockers::collect_state_storage_blockers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub object_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub data_bytes: usize,
    pub selected_instructions: usize,
    pub instruction_operands: usize,
    pub machine_code_bytes: usize,
    pub encoded_machine_bytes: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn build_emission_plan(native_plan: &NativePlan) -> EmissionPlan {
    let mut blockers = Arena::new();
    let (state_schedule, needs_runtime_dispatch) = match build_entry_state_schedule(native_plan) {
        Ok(state_schedule) => (state_schedule, false),
        Err(reason) => {
            if native_plan.runtime_dispatch_loop.needed {
                if !runtime_dispatch_loop_can_emit(native_plan) {
                    blockers.insert(runtime_dispatch_loop_blocker(native_plan));
                }
            } else {
                blockers.insert(blocker("state schedule", &reason));
                collect_runtime_dispatch_blockers(native_plan, &mut blockers);
            }
            (runtime_and_required_states(native_plan), true)
        }
    };

    if native_plan.machine_code.bytes.len() < native_plan.machine_code.byte_count {
        blockers.insert(blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
        if !scheduled_state_contains_key(&state_schedule, unsupported_call.source_key) {
            continue;
        }

        blockers.insert(blocker(
            "host lowering",
            &format!(
                "{}.{} statement {} platform call `{}`: {}",
                unsupported_call.machine,
                unsupported_call.state,
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason
            ),
        ));
    }

    collect_host_argument_blockers(native_plan, &state_schedule, &mut blockers);
    collect_state_call_blockers(
        native_plan,
        &state_schedule,
        needs_runtime_dispatch,
        &mut blockers,
    );
    collect_state_storage_blockers(native_plan, needs_runtime_dispatch, &mut blockers);
    if needs_runtime_dispatch {
        collect_state_guard_blockers(native_plan, &mut blockers);
        collect_state_value_blockers(native_plan, &mut blockers);
    }
    collect_state_codegen_blockers(native_plan, &state_schedule, &mut blockers);

    if !can_emit_real_object(native_plan) {
        blockers.insert_many([
            blocker(
                "relocation encoding",
                "planned relocation records are not serialized into this target object format yet",
            ),
            blocker(
                "object writer",
                "this target still falls back to the Omega native object container",
            ),
        ]);
    }

    EmissionPlan {
        object_format: native_plan.target.object_format,
        entry_symbol: native_plan.object.entry_symbol.clone(),
        sections: native_plan.object.sections.len(),
        symbols: native_plan.object.symbols.len(),
        host_bindings: native_plan.host_abi.bindings.len(),
        host_calls: native_plan.host_calls.calls.len(),
        data_bytes: native_plan.data.bytes.len(),
        selected_instructions: native_plan.instructions.instructions.len(),
        instruction_operands: native_plan.instructions.operands.len(),
        machine_code_bytes: native_plan.machine_code.byte_count,
        encoded_machine_bytes: native_plan.machine_code.bytes.len(),
        relocations: native_plan.relocations.records.len(),
        blockers,
    }
}

fn collect_host_argument_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::state_schedule::ScheduledState],
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
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{}.{} statement {} has an invalid argument span",
                    host_call.machine, host_call.state, host_call.statement_index
                ),
            ));
            continue;
        };
        let Some(first_argument) = arguments.first() else {
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{}.{} statement {} needs a text argument",
                    host_call.machine, host_call.state, host_call.statement_index
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
                    .map(host_text_argument_blocker_reason)
                    .unwrap_or_else(|| {
                        format!(
                            "{}.{} statement {} text argument `{}` needs runtime text lowering",
                            host_call.machine,
                            host_call.state,
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
        slot.place.display_name() == text_use.expression.display_name() && slot.has_input_buffer
    })
}

fn host_text_argument_blocker_reason(text_use: &RuntimeTextUse) -> String {
    let lowering_need = match text_use.source {
        RuntimeTextSource::StoredPlace => "runtime string storage lowering",
        RuntimeTextSource::GeneratedString => "runtime string builder lowering",
        RuntimeTextSource::MutablePlace => "runtime mutable string place lowering",
        RuntimeTextSource::OtherExpression => "runtime string expression lowering",
    };

    format!(
        "{}.{} statement {} text argument `{}` needs {lowering_need}",
        text_use.machine,
        text_use.state,
        text_use.statement_index,
        text_use.expression.display_name()
    )
}

fn can_emit_real_object(native_plan: &NativePlan) -> bool {
    can_emit_target_output(native_plan.target)
        && native_plan.machine_code.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}
