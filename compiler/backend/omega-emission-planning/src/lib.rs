use omega_artifacts::{EmissionBlocker, EmissionPlan, emission_blocker};
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::Arena;
use omega_image_emission::can_emit_executable_image;
use omega_layout::LayoutPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_program::MachineProgram;
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
use omega_target_operations::{InstructionPlan, TargetDataPlan};

mod host_argument_blockers;
mod host_binding_blockers;
mod runtime_dispatch_blockers;
mod runtime_text_blockers;
mod semantic_scope;
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
use semantic_scope::{proof_scope_suffix, state_name};

pub struct EmissionPlanningInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub host_abi: &'plan HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub state_values: &'plan StateValuePlan,
    pub data: &'plan TargetDataPlan,
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
    pub machine_program: &'plan MachineProgram,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub object: &'plan ObjectPlan,
    pub relocations: &'plan RelocationPlan,
}

pub fn build_emission_plan(input: &EmissionPlanningInput<'_>) -> EmissionPlan {
    let mut blockers = Arena::new();
    let schedule_context =
        StateScheduleContext::new(&input.control_flow, &input.host_calls, input.state_calls);
    let (state_schedule, needs_runtime_dispatch) =
        match build_entry_state_schedule(&schedule_context, input.entry_key) {
            Ok(state_schedule) => (state_schedule, false),
            Err(reason) => {
                if input.runtime_dispatch_loop.needed {
                    if !runtime_dispatch_loop_can_emit(input) {
                        blockers.insert(runtime_dispatch_loop_blocker(input));
                    }
                } else {
                    blockers.insert(emission_blocker("state schedule", &reason));
                    collect_runtime_dispatch_blockers(input, &mut blockers);
                }
                (runtime_and_required_states(input), true)
            }
        };

    if input.encoded_machine.instructions.len() < input.machine_program.instructions.len() {
        blockers.insert(emission_blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in input.host_calls.unsupported_calls.iter() {
        if !scheduled_state_contains_key(&state_schedule, unsupported_call.source_key) {
            continue;
        }

        blockers.insert(emission_blocker(
            "host lowering",
            &format!(
                "{} statement {} platform call `{}`: {}{}",
                state_name(input, unsupported_call.source_key),
                unsupported_call.statement_index,
                unsupported_call.platform_call,
                unsupported_call.reason,
                proof_scope_suffix(input, unsupported_call.source_key)
            ),
        ));
    }

    collect_host_binding_blockers(input, &mut blockers);
    collect_host_argument_blockers(input, &state_schedule, &mut blockers);
    collect_state_call_blockers(
        input,
        &state_schedule,
        needs_runtime_dispatch,
        &mut blockers,
    );
    collect_state_storage_blockers(input, needs_runtime_dispatch, &mut blockers);
    if needs_runtime_dispatch {
        collect_state_guard_blockers(input, &mut blockers);
        collect_state_value_blockers(input, &mut blockers);
    }
    collect_state_codegen_blockers(input, &schedule_context, &state_schedule, &mut blockers);

    if !can_emit_direct_image(input) {
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
        image_format: input.target.object_format,
        entry_symbol: input.object.entry_symbol.clone(),
        sections: input.object.sections.len(),
        symbols: input.object.symbols.len(),
        host_bindings: input.host_abi.bindings.len(),
        host_calls: input.host_calls.calls.len(),
        data_bytes: input.data.bytes.len(),
        selected_instructions: input.instructions.instructions.len(),
        instruction_operands: input.instructions.operands.len(),
        machine_code_bytes: input.encoded_machine.byte_count,
        encoded_machine_bytes: input.encoded_machine.bytes.len(),
        relocations: input.relocations.records.len(),
        blockers,
    }
}

fn can_emit_direct_image(input: &EmissionPlanningInput<'_>) -> bool {
    can_emit_executable_image(input.target)
        && input.encoded_machine.instructions.len() == input.machine_program.instructions.len()
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    emission_blocker(stage, reason)
}
