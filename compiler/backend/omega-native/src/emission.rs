use crate::abi::PlatformCallData;
use crate::control_flow::{OperationKind, StateFlow, StateKey};
use crate::host_calls::{HostCall, HostCallArgumentKind};
use crate::plan::NativePlan;
use crate::runtime_dispatch::loop_plan::RuntimeDispatchLoopAction;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::runtime_text::{RuntimeTextSource, RuntimeTextUse};
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::state_schedule::{
    build_entry_state_schedule, scheduled_state_contains_key, scheduled_state_flow,
    scheduled_state_key,
};
use crate::target::ObjectFormat;
use crate::target_output::can_emit_target_output;
use omega_core::arena::Arena;

mod runtime_text_blockers;
mod state_call_blockers;
mod storage_blockers;

use runtime_text_blockers::collect_state_value_blockers;
use state_call_blockers::collect_state_call_blockers;
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

fn collect_state_guard_blockers(native_plan: &NativePlan, blockers: &mut Arena<EmissionBlocker>) {
    for (_, guard) in native_plan.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp | StateGuardLowering::CompareStaticValue
        ) {
            continue;
        }

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}` needs runtime guard lowering",
                guard.source_dispatch_index,
                guard.source_machine,
                guard.source_state,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(&guard.target),
                guard.kind,
                guard.lowering,
                guard.expression.display_name()
            ),
        ));
    }
}

fn collect_state_codegen_blockers(
    native_plan: &NativePlan,
    state_schedule: &[crate::state_schedule::ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for scheduled_state in state_schedule {
        let Some(state_flow) = scheduled_state_flow(native_plan, scheduled_state) else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "scheduled state {}.{}#{} was not present in the control-flow plan",
                    scheduled_state.key.machine.arena_index(),
                    scheduled_state.key.state.arena_index(),
                    scheduled_state.key.segment_index
                ),
            ));
            continue;
        };
        let machine_name =
            machine_name_for_state(native_plan, state_flow).unwrap_or("<missing-machine>");
        let state_name = state_flow.name.as_str();

        let Some(operations) = native_plan
            .control_flow
            .operations
            .span(state_flow.operations)
        else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "{}.{} has an invalid operation span",
                    machine_name, state_name
                ),
            ));
            continue;
        };

        for operation in operations {
            match operation.kind {
                OperationKind::Call { .. }
                    if state_statement_has_host_call(
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) || state_statement_has_state_call(
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                OperationKind::Call { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} is a call that is not lowered to a native host operation",
                            machine_name,
                            state_name,
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::ConstantIntegerAssignment
                | OperationKind::StaticAssignment { .. } => {}
                OperationKind::Assignment { .. }
                    if state_statement_has_storage_mutation(
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                OperationKind::Assignment { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} Assignment is not supported by native emission yet",
                            machine_name, state_name, operation.statement_index
                        ),
                    ));
                }
                OperationKind::LocalData
                    if state_statement_has_local_storage(
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                _ => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{}.{} statement {} {:?} is not supported by native emission yet",
                            machine_name, state_name, operation.statement_index, operation.kind
                        ),
                    ));
                }
            };
        }
    }
}

fn state_statement_has_local_storage(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.state_storage.locals.iter().any(|(_, local)| {
        local.source_key == source_key && local.statement_index == statement_index
    })
}

fn state_statement_has_storage_mutation(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan
        .state_storage
        .mutations
        .iter()
        .any(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
}

fn state_statement_has_state_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.source_key == source_key && state_call.statement_index == statement_index
    })
}

fn runtime_and_required_states(
    native_plan: &NativePlan,
) -> Vec<crate::state_schedule::ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(native_plan, &mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state(
                native_plan,
                &mut states,
                &state_call.source_machine,
                &state_call.source_state,
            );

            if !state_call.target_machine.is_empty() {
                push_scheduled_state(
                    native_plan,
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
    native_plan: &NativePlan,
    states: &mut Vec<crate::state_schedule::ScheduledState>,
    machine: &str,
    state: &str,
) {
    let Some(key) = scheduled_state_key(native_plan, machine, state) else {
        return;
    };

    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(crate::state_schedule::ScheduledState { key });
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

fn runtime_dispatch_loop_blocker(native_plan: &NativePlan) -> EmissionBlocker {
    if let Some(guard_lowering) = first_unsupported_dispatch_guard(native_plan) {
        return blocker(
            "runtime dispatch",
            &format!(
                "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); guard lowering {guard_lowering:?} needs runtime state comparison byte emission",
                native_plan.runtime_dispatch_loop.cases.len(),
                native_plan.runtime_dispatch_loop.edges.len(),
                native_plan.runtime_flow.cycles.len()
            ),
        );
    }

    blocker(
        "runtime dispatch",
        &format!(
            "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); native emission needs dispatch loop byte emission",
            native_plan.runtime_dispatch_loop.cases.len(),
            native_plan.runtime_dispatch_loop.edges.len(),
            native_plan.runtime_flow.cycles.len()
        ),
    )
}

fn runtime_dispatch_loop_can_emit(native_plan: &NativePlan) -> bool {
    native_plan
        .runtime_dispatch_loop
        .edges
        .iter()
        .all(|(_, edge)| {
            dispatch_loop_guard_can_emit(edge) && edge.action != RuntimeDispatchLoopAction::Unknown
        })
}

fn first_unsupported_dispatch_guard(native_plan: &NativePlan) -> Option<StateGuardLowering> {
    native_plan
        .runtime_dispatch_loop
        .edges
        .iter()
        .find(|(_, edge)| !dispatch_loop_guard_can_emit(edge))
        .map(|(_, edge)| edge.guard_lowering)
}

fn dispatch_loop_guard_can_emit(
    edge: &crate::runtime_dispatch::loop_plan::RuntimeDispatchLoopEdge,
) -> bool {
    match edge.guard_lowering {
        StateGuardLowering::NoOp => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    StateGuardOperator::Equal | StateGuardOperator::NotEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4)
        }
        StateGuardLowering::CompareRuntimeValue | StateGuardLowering::NeedsRuntimeExpression => {
            false
        }
    }
}

fn runtime_transition_target_name(target: &RuntimeTransitionTarget) -> String {
    match target {
        RuntimeTransitionTarget::State { machine, state, .. } => format!("{machine}.{state}"),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown { name } => format!("unknown {name}"),
    }
}

fn machine_name_for_state<'plan>(
    native_plan: &'plan NativePlan,
    state_flow: &StateFlow,
) -> Option<&'plan str> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_flow.key.machine)
        .map(|(_, machine)| machine.name.as_str())
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
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
