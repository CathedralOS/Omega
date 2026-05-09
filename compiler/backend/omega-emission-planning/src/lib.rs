use omega_artifacts::{EmissionBlocker, EmissionPlan, emission_blocker};
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_image_emission::can_emit_executable_image;
use omega_layout::LayoutPlan;
use omega_machine_program::{EncodedMachinePlan, MachineCodePlan};
use omega_object::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_contains_key,
};
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_program::{InstructionPlan, NativeDataPlan};

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

pub struct EmissionPlanningInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub host_abi: &'plan HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub state_values: &'plan StateValuePlan,
    pub data: &'plan NativeDataPlan,
    pub instructions: &'plan InstructionPlan,
    pub control_flow: &'plan ControlFlowPlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub layouts: &'plan LayoutPlan,
    pub machine_code: &'plan MachineCodePlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub object: &'plan ObjectPlan,
    pub relocations: &'plan RelocationPlan,
}

pub fn build_emission_plan(native_plan: &EmissionPlanningInput<'_>) -> EmissionPlan {
    let mut blockers = Arena::new();
    let schedule_context =
        StateScheduleContext::new(&native_plan.control_flow, &native_plan.host_calls);
    let (state_schedule, needs_runtime_dispatch) =
        match build_entry_state_schedule(&schedule_context, native_plan.entry_key) {
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

    if native_plan.encoded_machine.bytes.len() < native_plan.machine_code.byte_count {
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
    collect_state_codegen_blockers(
        native_plan,
        &schedule_context,
        &state_schedule,
        &mut blockers,
    );

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
        encoded_machine_bytes: native_plan.encoded_machine.bytes.len(),
        relocations: native_plan.relocations.records.len(),
        blockers,
    }
}

fn can_emit_direct_image(native_plan: &EmissionPlanningInput<'_>) -> bool {
    can_emit_executable_image(native_plan.target)
        && native_plan.encoded_machine.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    emission_blocker(stage, reason)
}

fn state_name(
    native_plan: &EmissionPlanningInput<'_>,
    key: omega_control_flow::StateKey,
) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
