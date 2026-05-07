use crate::native::abi::PlatformCallData;
use crate::native::control_flow::{OperationKind, StateFlow};
use crate::native::host_calls::HostCallArgumentKind;
use crate::native::plan::NativePlan;
use crate::native::platform_object::can_emit_target_object;
use crate::native::runtime_flow::RuntimeTransitionTarget;
use crate::native::state_calls::StateCallLowering;
use crate::native::state_guards::StateGuardKind;
use crate::native::state_schedule::{build_entry_state_schedule, scheduled_state_contains};
use crate::native::state_storage::StateMutationLowering;
use crate::native::state_values::{StateValueKind, StateValueRole};
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
    let (state_schedule, needs_runtime_dispatch) = match build_entry_state_schedule(native_plan) {
        Ok(state_schedule) => (state_schedule, false),
        Err(reason) => {
            blockers.insert(blocker("state schedule", &reason));
            collect_runtime_dispatch_blockers(native_plan, &mut blockers);
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
    collect_state_call_blockers(
        native_plan,
        &state_schedule,
        needs_runtime_dispatch,
        &mut blockers,
    );
    collect_state_storage_blockers(native_plan, &mut blockers);
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

fn collect_state_call_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::native::state_schedule::ScheduledState],
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        if state_call.target_machine.is_empty() {
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls unresolved state `{}` through `{}`",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_state,
                    state_call.receiver
                ),
            ));
            continue;
        }

        if matches!(
            state_call.lowering,
            StateCallLowering::InlineLeaf | StateCallLowering::InlineExpansion
        ) && !needs_runtime_dispatch
            && scheduled_state_contains(
                state_schedule,
                &state_call.source_machine,
                &state_call.source_state,
            )
            && scheduled_state_contains(
                state_schedule,
                &state_call.target_machine,
                &state_call.target_state,
            )
        {
            continue;
        }

        match state_call.lowering {
            StateCallLowering::InlineLeaf => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls leaf state {}.{} with {} argument(s); native emission needs leaf state-call inlining",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_machine,
                    state_call.target_state,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::InlineExpansion => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls {}.{} with {} argument(s); native emission needs inline state-call expansion",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_machine,
                    state_call.target_state,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::Unresolved => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{}.{} statement {} calls unresolved state `{}` through `{}`",
                    state_call.source_machine,
                    state_call.source_state,
                    state_call.statement_index,
                    state_call.target_state,
                    state_call.receiver
                ),
            )),
        };
    }
}

fn collect_state_value_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
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

        blockers.insert(blocker(
            "state values",
            &format!(
                "{}.{} statement {} {:?} binary expression `{}` needs runtime value lowering",
                value.machine,
                value.state,
                value.statement_index,
                value.role,
                value.expression.display_name()
            ),
        ));
    }
}

fn collect_state_guard_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        if guard.kind == StateGuardKind::Always {
            continue;
        }

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?} `{}` needs runtime guard lowering",
                guard.source_dispatch_index,
                guard.source_machine,
                guard.source_state,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(&guard.target),
                guard.kind,
                guard.expression.display_name()
            ),
        ));
    }
}

fn state_value_is_static_assignment(
    native_plan: &NativePlan,
    value: &crate::native::state_values::StateValueUse,
) -> bool {
    if value.role != crate::native::state_values::StateValueRole::AssignmentValue {
        return false;
    }
    let Some(state) = state_flow(native_plan, &value.machine, &value.state) else {
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

fn collect_state_storage_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
    for (_, local) in native_plan.state_storage.locals.iter() {
        if !local.required {
            continue;
        }

        blockers.insert(blocker(
            "state storage",
            &format!(
                "{}.{} statement {} local `{}`: {} needs stack/local storage lowering",
                local.machine, local.state, local.statement_index, local.name, local.type_name
            ),
        ));
    }

    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        if mutation.lowering == StateMutationLowering::AlreadyLowered {
            continue;
        }

        blockers.insert(blocker(
            "state mutation",
            &format!(
                "{}.{} statement {} {:?}/{:?} `{}` = `{}` needs mutation lowering",
                mutation.machine,
                mutation.state,
                mutation.statement_index,
                mutation.mutation_kind,
                mutation.lowering,
                mutation.target.display_name(),
                mutation.value.display_name()
            ),
        ));
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
                    ) || state_statement_has_state_call(
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
                OperationKind::Assignment { .. }
                    if state_statement_has_storage_mutation(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
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
                OperationKind::LocalData
                    if state_statement_has_local_storage(
                        native_plan,
                        &scheduled_state.machine,
                        &scheduled_state.state,
                        operation.statement_index,
                    ) => {}
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

fn state_statement_has_local_storage(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.state_storage.locals.iter().any(|(_, local)| {
        local.machine == machine_name
            && local.state == state_name
            && local.statement_index == statement_index
    })
}

fn state_statement_has_storage_mutation(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan
        .state_storage
        .mutations
        .iter()
        .any(|(_, mutation)| {
            mutation.machine == machine_name
                && mutation.state == state_name
                && mutation.statement_index == statement_index
        })
}

fn state_statement_has_state_call(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> bool {
    native_plan.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.source_machine == machine_name
            && state_call.source_state == state_name
            && state_call.statement_index == statement_index
    })
}

fn runtime_and_required_states(
    native_plan: &NativePlan,
) -> Vec<crate::native::state_schedule::ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(&mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state(
                &mut states,
                &state_call.source_machine,
                &state_call.source_state,
            );

            if !state_call.target_machine.is_empty() {
                push_scheduled_state(
                    &mut states,
                    &state_call.target_machine,
                    &state_call.target_state,
                );
            }
        }
    }

    states
}

fn push_scheduled_state(
    states: &mut Vec<crate::native::state_schedule::ScheduledState>,
    machine: &str,
    state: &str,
) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.machine == machine && scheduled_state.state == state)
    {
        return;
    }

    states.push(crate::native::state_schedule::ScheduledState {
        machine: machine.to_owned(),
        state: state.to_owned(),
    });
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

fn runtime_transition_target_name(target: &RuntimeTransitionTarget) -> String {
    match target {
        RuntimeTransitionTarget::State { machine, state } => format!("{machine}.{state}"),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
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
