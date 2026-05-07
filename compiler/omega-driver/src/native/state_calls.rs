use crate::native::control_flow::{ControlFlowPlan, MachineFlow, OperationKind};
use crate::native::plan::NativePlan;
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlan {
    pub calls: Arena<StateCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCall {
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub receiver: String,
    pub target_machine: String,
    pub target_state: String,
    pub argument_count: usize,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
}

impl Default for StateCall {
    fn default() -> Self {
        Self {
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            receiver: String::new(),
            target_machine: String::new(),
            target_state: String::new(),
            argument_count: 0,
            reachable: false,
            required: false,
            resolution: StateCallResolution::Unresolved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StateCallResolution {
    Local,
    ContainedMachine,
    NamedMachine,
    #[default]
    Unresolved,
}

pub fn build_state_call_plan(native_plan: &NativePlan) -> StateCallPlan {
    let mut calls = Vec::new();

    for (_, machine) in native_plan.control_flow.machines.iter() {
        collect_machine_state_calls(native_plan, machine, &mut calls);
    }

    mark_required_state_calls(native_plan, &mut calls);

    let mut plan = StateCallPlan::default();
    plan.calls.insert_many(calls);
    plan
}

fn collect_machine_state_calls(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    calls: &mut Vec<StateCall>,
) {
    let Some(states) = native_plan.control_flow.states.span(machine.states) else {
        return;
    };

    for state in states {
        let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
            continue;
        };

        for operation in operations {
            let OperationKind::Call {
                receiver,
                target,
                arguments,
            } = &operation.kind
            else {
                continue;
            };

            if state_statement_has_host_call(
                native_plan,
                &machine.name,
                &state.name,
                operation.statement_index,
            ) {
                continue;
            }

            let resolved_target = resolve_state_call_target(
                &native_plan.control_flow,
                machine,
                receiver.as_deref(),
                target,
            );

            calls.push(StateCall {
                source_machine: machine.name.clone(),
                source_state: state.name.clone(),
                statement_index: operation.statement_index,
                receiver: receiver.clone().unwrap_or_else(|| "self".to_owned()),
                target_machine: resolved_target
                    .as_ref()
                    .map(|target| target.machine.clone())
                    .unwrap_or_default(),
                target_state: target.clone(),
                argument_count: arguments.len(),
                reachable: runtime_state_is_reachable(native_plan, &machine.name, &state.name),
                required: false,
                resolution: resolved_target
                    .map(|target| target.resolution)
                    .unwrap_or(StateCallResolution::Unresolved),
            });
        }
    }
}

fn mark_required_state_calls(native_plan: &NativePlan, calls: &mut [StateCall]) {
    let mut required_states = native_plan
        .runtime_flow
        .states
        .iter()
        .map(|(_, state)| (state.machine.clone(), state.state.clone()))
        .collect::<Vec<_>>();
    let mut changed = true;

    while changed {
        changed = false;

        for call in calls.iter_mut() {
            let source_is_required = required_states.iter().any(|(machine, state)| {
                machine == &call.source_machine && state == &call.source_state
            });

            if !source_is_required {
                continue;
            }

            if !call.required {
                call.required = true;
                changed = true;
            }

            if call.target_machine.is_empty() {
                continue;
            }

            let target = (call.target_machine.clone(), call.target_state.clone());
            if !required_states.contains(&target) {
                required_states.push(target);
                changed = true;
            }
        }
    }
}

struct ResolvedStateCall {
    machine: String,
    resolution: StateCallResolution,
}

fn resolve_state_call_target(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    receiver: Option<&str>,
    target_state: &str,
) -> Option<ResolvedStateCall> {
    let Some(receiver) = receiver else {
        if machine_has_state(control_flow, &machine.name, target_state) {
            return Some(ResolvedStateCall {
                machine: machine.name.clone(),
                resolution: StateCallResolution::Local,
            });
        }

        return None;
    };

    if receiver == "self" && machine_has_state(control_flow, &machine.name, target_state) {
        return Some(ResolvedStateCall {
            machine: machine.name.clone(),
            resolution: StateCallResolution::Local,
        });
    }

    if let Some(contained) = machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
    {
        return machine_has_state(control_flow, &contained.type_name, target_state).then(|| {
            ResolvedStateCall {
                machine: contained.type_name.clone(),
                resolution: StateCallResolution::ContainedMachine,
            }
        });
    }

    machine_has_state(control_flow, receiver, target_state).then(|| ResolvedStateCall {
        machine: receiver.to_owned(),
        resolution: StateCallResolution::NamedMachine,
    })
}

fn machine_has_state(control_flow: &ControlFlowPlan, machine_name: &str, state_name: &str) -> bool {
    control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| control_flow.states.span(machine.states))
        .is_some_and(|states| states.iter().any(|state| state.name == state_name))
}

fn runtime_state_is_reachable(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> bool {
    native_plan
        .runtime_flow
        .states
        .iter()
        .any(|(_, state)| state.machine == machine_name && state.state == state_name)
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
