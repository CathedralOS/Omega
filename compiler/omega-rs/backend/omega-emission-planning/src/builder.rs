use crate::EmissionPlanningInput;
use crate::authored_import_blockers::collect_authored_import_blockers;
use crate::contained_receiver_blockers::collect_contained_receiver_blockers;
use crate::descriptor_argument_blockers::collect_descriptor_argument_blockers;
use crate::host_argument_blockers::collect_host_argument_blockers;
use crate::host_binding_blockers::collect_host_binding_blockers;
use crate::reentrant_value_call_blockers::collect_reentrant_value_call_blockers;
use crate::required_emission_verification::verify_required_items_emitted;
use crate::runtime_dispatch_blockers::{
    collect_runtime_dispatch_blockers, runtime_and_required_states, runtime_dispatch_loop_blocker,
    runtime_dispatch_loop_can_emit,
};
use crate::runtime_text_blockers::collect_state_value_blockers;
use crate::semantic_scope::{proof_scope_suffix, state_name};
use crate::state_call_blockers::collect_state_call_blockers;
use crate::state_codegen_blockers::collect_state_codegen_blockers;
use crate::state_guard_blockers::collect_state_guard_blockers;
use crate::storage_blockers::collect_state_storage_blockers;
use crate::unlowered_guard_blockers::collect_unlowered_guard_blockers;
use crate::value_call_arm_effect_blockers::collect_value_call_arm_effect_blockers;
use omega_backend_report_types::{EmissionPlan, emission_blocker};
use omega_image_emission::can_emit_executable_image;
use omega_platform_interface::UnsupportedHostCallReason;
use omega_state_schedule::{
    StateScheduleContext, build_entry_state_schedule, scheduled_state_contains_key,
};
use psi_arena::Arena;

pub fn build_emission_plan(input: &EmissionPlanningInput<'_>) -> EmissionPlan {
    let mut blockers = Arena::with_capacity(estimated_emission_blocker_capacity(input));
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

    if input.encoded_machine.code.instructions.len()
        < input.machine_instructions.code.instructions.len()
    {
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
                unsupported_host_call_reason_text(unsupported_call.reason),
                proof_scope_suffix(input, unsupported_call.source_key)
            ),
        ));
    }

    collect_host_binding_blockers(input, &mut blockers);
    collect_host_argument_blockers(input, &state_schedule, &mut blockers);
    collect_contained_receiver_blockers(input, &mut blockers);
    collect_value_call_arm_effect_blockers(input, &mut blockers);
    collect_reentrant_value_call_blockers(input, &mut blockers);
    crate::call_result_blockers::collect_call_result_return_blockers(input, &mut blockers);
    collect_authored_import_blockers(input, &mut blockers);
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
    collect_unlowered_guard_blockers(input, &mut blockers);

    verify_required_items_emitted(input, needs_runtime_dispatch, &mut blockers);
    collect_descriptor_argument_blockers(input, needs_runtime_dispatch, &mut blockers);

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
        entry_symbol: omega_object_file::object_entry_symbol_name(input.object).to_owned(),
        sections: input.object.layout.sections.len(),
        symbols: input.object.layout.symbols.len(),
        host_bindings: input.host_abi.bindings.len(),
        host_calls: input.host_calls.calls.len(),
        data_bytes: input.data.bytes.len(),
        selected_instructions: input.instructions.code.instructions.len(),
        instruction_operands: input.instructions.code.operands.len(),
        machine_code_bytes: input.encoded_machine.code.byte_count,
        encoded_machine_bytes: input.encoded_machine.code.bytes.len(),
        relocations: input.relocations.record_set.records.len(),
        blockers,
    }
}

fn estimated_emission_blocker_capacity(input: &EmissionPlanningInput<'_>) -> usize {
    let fixed_blockers = 6usize;

    fixed_blockers
        .saturating_add(input.host_calls.unsupported_calls.len())
        .saturating_add(input.host_abi.bindings.len())
        .saturating_add(input.host_calls.calls.len())
        .saturating_add(input.state_calls.calls.len().saturating_mul(2))
        .saturating_add(input.state_storage.locals.len())
        .saturating_add(input.state_storage.mutations.len())
        .saturating_add(input.state_values.values.len())
        .saturating_add(input.runtime_flow.cycles.len())
        .saturating_add(input.runtime_bodies.operations.len())
        .saturating_add(input.runtime_storage.frame_slots.len())
        .saturating_add(input.runtime_storage.writes.len())
        .saturating_add(input.runtime_text.uses.len())
        .saturating_add(input.runtime_text.writes.len())
        .saturating_add(input.state_guards.guards.len())
        .saturating_add(input.control_flow.operations.len())
}

fn can_emit_direct_image(input: &EmissionPlanningInput<'_>) -> bool {
    can_emit_executable_image(input.target)
        && input.encoded_machine.code.instructions.len()
            == input.machine_instructions.code.instructions.len()
}

fn unsupported_host_call_reason_text(reason: UnsupportedHostCallReason) -> String {
    match reason {
        UnsupportedHostCallReason::NoNativeLowering { target } => {
            format!("no native lowering for target {target:?}")
        }
    }
}
