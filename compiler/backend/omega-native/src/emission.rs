use crate::plan::NativePlan;
use crate::state_schedule::{build_entry_state_schedule, scheduled_state_contains_key};
use crate::target_output::can_emit_target_output;
use omega_artifacts::{EmissionBlocker, EmissionPlan, emission_blocker};
use omega_core::arena::Arena;

mod host_argument_blockers;
mod host_binding_blockers;
mod runtime_dispatch_blockers;
mod runtime_text_blockers;
mod state_call_blockers;
mod state_codegen_blockers;
mod state_guard_blockers;
mod storage_blockers;

use host_argument_blockers::collect_host_argument_blockers;
use host_binding_blockers::collect_host_binding_blockers;
use runtime_dispatch_blockers::{
    collect_runtime_dispatch_blockers, runtime_and_required_states, runtime_dispatch_loop_blocker,
    runtime_dispatch_loop_can_emit,
};
use runtime_text_blockers::collect_state_value_blockers;
use state_call_blockers::collect_state_call_blockers;
use state_codegen_blockers::collect_state_codegen_blockers;
use state_guard_blockers::collect_state_guard_blockers;
use storage_blockers::collect_state_storage_blockers;

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
                blockers.insert(emission_blocker("state schedule", &reason));
                collect_runtime_dispatch_blockers(native_plan, &mut blockers);
            }
            (runtime_and_required_states(native_plan), true)
        }
    };

    if native_plan.machine_code.bytes.len() < native_plan.machine_code.byte_count {
        blockers.insert(emission_blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
        if !scheduled_state_contains_key(&state_schedule, unsupported_call.source_key) {
            continue;
        }

        blockers.insert(emission_blocker(
            "host lowering",
            &format!(
                "{} statement {} platform call `{}`: {}",
                state_name(native_plan, unsupported_call.source_key),
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason
            ),
        ));
    }

    collect_host_binding_blockers(native_plan, &mut blockers);
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

    if !can_emit_direct_image(native_plan) {
        blockers.insert_many([
            emission_blocker(
                "image writer",
                "no direct executable image writer is registered for this target",
            ),
            emission_blocker(
                "image relocation",
                "this target cannot apply final image relocations without a direct image writer",
            ),
        ]);
    }

    EmissionPlan {
        image_format: native_plan.target.object_format,
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

fn can_emit_direct_image(native_plan: &NativePlan) -> bool {
    can_emit_target_output(native_plan.target)
        && native_plan.machine_code.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    emission_blocker(stage, reason)
}

fn state_name(native_plan: &NativePlan, key: omega_control_flow::StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
