use crate::native::abi::PlatformCallData;
use crate::native::control_flow::{OperationKind, StateFlow};
use crate::native::host_calls::HostCallArgumentKind;
use crate::native::plan::NativePlan;
use crate::native::platform_object::can_emit_target_object;
use crate::native::state_schedule::{build_entry_state_schedule, scheduled_state_contains};
use crate::native::target::ObjectFormat;
use omega_core::arena::Arena;

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
    let state_schedule = match build_entry_state_schedule(native_plan) {
        Ok(state_schedule) => state_schedule,
        Err(reason) => {
            blockers.insert(blocker("state schedule", &reason));
            collect_runtime_dispatch_blockers(native_plan, &mut blockers);
            runtime_reachable_states(native_plan)
        }
    };

    if native_plan.machine_code.bytes.len() < native_plan.machine_code.byte_count {
        blockers.insert(blocker(
            "machine encoding",
            "not all selected native instructions are encoded into target bytes yet",
        ));
    }

    for (_, unsupported_call) in native_plan.host_calls.unsupported_calls.iter() {
        if !scheduled_state_contains(
            &state_schedule,
            &unsupported_call.machine,
            &unsupported_call.state,
        ) {
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
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, host_call) in native_plan.host_calls.calls.iter() {
        if !scheduled_state_contains(state_schedule, &host_call.machine, &host_call.state) {
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
            blockers.insert(blocker(
                "host arguments",
                &format!(
                    "{}.{} statement {} text argument `{expression}` needs runtime string lowering",
                    host_call.machine, host_call.state, host_call.statement_index
                ),
            ));
        }
    }
}

fn collect_state_codegen_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for scheduled_state in state_schedule {
        let Some(state_flow) = state_flow(
            native_plan,
            &scheduled_state.machine,
            &scheduled_state.state,
        ) else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "scheduled state {}.{} was not present in the control-flow plan",
                    scheduled_state.machine, scheduled_state.state
                ),
            ));
            continue;
        };

        let Some(operations) = native_plan
            .control_flow
            .operations
            .span(state_flow.operations)
        else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "{}.{} has an invalid operation span",
                    scheduled_state.machine, scheduled_state.state
                ),
            ));
            continue;
        };

        for operation in operations {
            match operation.kind {
                OperationKind::Call { .. }
                    if state_statement_has_host_call(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
                OperationKind::Call { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} is a call that is not lowered to a native host operation",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::ConstantIntegerAssignment
                | OperationKind::StaticAssignment { .. } => {}
                OperationKind::Assignment { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} Assignment is not supported by native emission yet",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index
                        ),
                    ));
                }
                _ => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} {:?} is not supported by native emission yet",
                            scheduled_state.machine,
                            scheduled_state.state,
                            operation.statement_index,
                            operation.kind
                        ),
                    ));
                }
            };
        }
    }
}

fn runtime_reachable_states(
    native_plan: &NativePlan,
) -> Vec<crate::native::state_schedule::ScheduledState> {
    native_plan
        .runtime_flow
        .states
        .iter()
        .map(|(_, state)| crate::native::state_schedule::ScheduledState {
            machine: state.machine.clone(),
            state: state.state.clone(),
        })
        .collect()
}

fn collect_runtime_dispatch_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, cycle) in native_plan.runtime_flow.cycles.iter() {
        let Some(states) = native_plan.runtime_flow.cycle_states.span(cycle.states) else {
            blockers.insert(blocker(
                "runtime dispatch",
                "invalid runtime cycle span in native flow plan",
            ));
            continue;
        };
        let cycle_path = states
            .iter()
            .map(|state| format!("{}.{}", state.machine, state.state))
            .collect::<Vec<_>>()
            .join(" -> ");

        blockers.insert(blocker(
            "runtime dispatch",
            &format!("cycle {cycle_path} needs generated state dispatch before native emission"),
        ));
    }
}

fn state_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<&'plan StateFlow> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.machine == machine_name
            && host_call.state == state_name
            && host_call.statement_index == statement_index
    })
}

fn can_emit_real_object(native_plan: &NativePlan) -> bool {
    can_emit_target_object(native_plan.target)
        && native_plan.machine_code.bytes.len() == native_plan.machine_code.byte_count
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}
