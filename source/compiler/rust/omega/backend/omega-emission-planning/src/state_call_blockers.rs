use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_state_calls::StateCall;
use omega_state_calls::StateCallLowering;
use omega_state_schedule::{ScheduledState, scheduled_state_contains_key};
use psi_arena::Arena;

use super::semantic_scope::{proof_scope_suffix, state_call_receiver_name, state_name};
use super::{EmissionBlocker, blocker};

mod runtime_body;

use runtime_body::collect_runtime_body_state_call_blockers;

pub(super) fn collect_state_call_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_state_call_blockers(input, blockers);
        collect_unresolved_state_call_blockers(input, blockers);
        return;
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        let source_name = state_name(input, state_call.source_key);
        if !state_call.target_key.is_valid() {
            if unresolved_call_is_host_call(input, state_call)
                || unresolved_call_is_wire_encode(input, state_call)
                || unresolved_call_is_asm_intrinsic(input, state_call)
            {
                continue;
            }
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} has unresolved state call through `{}`",
                    source_name,
                    state_call.statement_index,
                    state_call_receiver_name(input, state_call)
                ),
            ));
            continue;
        }
        let target_name = state_name(input, state_call.target_key);

        if state_call.lowering == StateCallLowering::IndirectDynamic
            && state_call.dynamic_dispatch.is_some()
        {
            continue;
        }

        if matches!(
            state_call.lowering,
            StateCallLowering::InlineLeaf
                | StateCallLowering::InlineBranching
                | StateCallLowering::InlineExpansion
        ) && !needs_runtime_dispatch
            && scheduled_state_contains_key(state_schedule, state_call.source_key)
            && scheduled_state_contains_key(state_schedule, state_call.target_key)
        {
            continue;
        }

        match state_call.lowering {
            StateCallLowering::InlineLeaf => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls leaf state {} with {} argument(s){}; native emission needs leaf state-call inlining",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
            StateCallLowering::InlineBranching => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls branching state {} with {} argument(s){}; native emission needs guarded state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
            StateCallLowering::InlineExpansion => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls {} with {} argument(s){}; native emission needs inline state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
        StateCallLowering::IndirectDynamic => blockers.insert(blocker(
            "state calls",
            "dynamic state call lost its exact runtime descriptor lowering",
        )),
        StateCallLowering::Unresolved => blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {} has unresolved state call through `{}`",
                source_name,
                state_call.statement_index,
                state_call_receiver_name(input, state_call)
            ),
        )),
        };
    }
}

fn collect_unresolved_state_call_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required || state_call.target_key.is_valid() {
            continue;
        }
        if unresolved_call_is_host_call(input, state_call)
            || unresolved_call_is_wire_encode(input, state_call)
            || unresolved_call_is_asm_intrinsic(input, state_call)
        {
            continue;
        }

        let source_name = state_name(input, state_call.source_key);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {} has unresolved state call through `{}`",
                source_name,
                state_call.statement_index,
                state_call_receiver_name(input, state_call)
            ),
        ));
    }
}

/// An asm intrinsic lowered into a raw machine instruction, not a state
/// transition, so its call record is legitimately unresolved.
fn unresolved_call_is_asm_intrinsic(
    input: &EmissionPlanningInput<'_>,
    state_call: &StateCall,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            matches!(
                instruction.kind,
                omega_target_operations::TargetOperationKind::MachineHalt
                    | omega_target_operations::TargetOperationKind::MemoryFence(_)
                    | omega_target_operations::TargetOperationKind::InterruptControl(_)
                    | omega_target_operations::TargetOperationKind::FlagsSnapshot { .. }
                    | omega_target_operations::TargetOperationKind::FlagsRestore { .. }
                    | omega_target_operations::TargetOperationKind::MsrRead { .. }
                    | omega_target_operations::TargetOperationKind::MsrWrite { .. }
                    | omega_target_operations::TargetOperationKind::ControlRegisterRead { .. }
                    | omega_target_operations::TargetOperationKind::ControlRegisterWrite { .. }
                    | omega_target_operations::TargetOperationKind::PortWrite { .. }
                    | omega_target_operations::TargetOperationKind::PortRead { .. }
            ) && state_key_matches_statement_source(instruction.source_key, state_call.source_key)
                && instruction.source_statement == state_call.statement_index
        })
}

/// A statement that lowered into the wire append/read families is the
/// synthesized `Schema::encode` / `Schema::decode` call, not an
/// unresolved state call.
fn unresolved_call_is_wire_encode(
    input: &EmissionPlanningInput<'_>,
    state_call: &StateCall,
) -> bool {
    statement_has_wire_encode_lowering(input, state_call.source_key, state_call.statement_index)
}

pub(super) fn statement_has_wire_encode_lowering(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            matches!(
                instruction.kind,
                omega_target_operations::TargetOperationKind::AppendWireLiteralByte { .. }
                    | omega_target_operations::TargetOperationKind::AppendWireScalarVarint { .. }
                    | omega_target_operations::TargetOperationKind::AppendWireTextBytes { .. }
                    | omega_target_operations::TargetOperationKind::AppendWireScalarSlice { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireExpectedByte { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireScalarVarint { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireByteSlice { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireNestedOpen { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireNestedClose { .. }
                    | omega_target_operations::TargetOperationKind::AppendWireRepeatedScalarVarint { .. }
                    | omega_target_operations::TargetOperationKind::ReadWireRepeatedScalarVarint { .. }
            ) && state_key_matches_statement_source(instruction.source_key, source_key)
                && instruction.source_statement == statement_index
        })
}

fn unresolved_call_is_host_call(input: &EmissionPlanningInput<'_>, state_call: &StateCall) -> bool {
    input.host_calls.calls.iter().any(|(_, host_call)| {
        state_key_matches_statement_source(host_call.source_key, state_call.source_key)
            && host_call.statement_index == state_call.statement_index
    })
}

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}
