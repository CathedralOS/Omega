use crate::control_flow::{ControlFlowPlan, MachineFlow, OperationKind};
use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_analysis::StateAnalysisContext;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::expression::Expression;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlan {
    pub calls: Arena<StateCall>,
    pub arguments: Arena<StateCallArgument>,
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
    pub arguments: HandleSpan<StateCallArgument>,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
    pub lowering: StateCallLowering,
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
            arguments: HandleSpan::empty(),
            reachable: false,
            required: false,
            resolution: StateCallResolution::Unresolved,
            lowering: StateCallLowering::Unresolved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCallArgument {
    pub index: usize,
    pub parameter_name: String,
    pub expression: Expression,
    pub kind: StateCallArgumentKind,
    pub required: bool,
}

impl Default for StateCallArgument {
    fn default() -> Self {
        Self {
            index: 0,
            parameter_name: String::new(),
            expression: Expression::Integer(0),
            kind: StateCallArgumentKind::Value,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateCallArgumentKind {
    #[default]
    Value,
    MutableAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StateCallResolution {
    Local,
    ContainedMachine,
    NamedMachine,
    #[default]
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateCallLowering {
    InlineLeaf,
    InlineBranching,
    InlineExpansion,
    #[default]
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CollectedStateCall {
    source_machine: String,
    source_state: String,
    statement_index: usize,
    receiver: String,
    target_machine: String,
    target_state: String,
    raw_arguments: Vec<Expression>,
    reachable: bool,
    required: bool,
    resolution: StateCallResolution,
}

pub fn build_state_call_plan(native_plan: &NativePlan) -> StateCallPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_state_call_plan_with_workers(
        Arc::new(StateAnalysisContext::from_native_plan(native_plan)),
        workers.handle(),
    )
}

pub fn build_state_call_plan_with_workers(
    context: Arc<StateAnalysisContext>,
    workers: WorkerPoolHandle,
) -> StateCallPlan {
    let machines = Arc::new(
        context
            .control_flow
            .machines
            .iter()
            .map(|(_, machine)| machine.clone())
            .collect::<Vec<_>>(),
    );
    let machine_count = machines.len();
    let context_for_collection = Arc::clone(&context);
    let machine_calls = workers.map_ordered(machine_count, move |index| {
        let machine = machines
            .get(index)
            .expect("state-call worker index should be in range");

        collect_machine_state_calls(&context_for_collection, machine)
    });

    let mut calls = machine_calls.into_iter().flatten().collect::<Vec<_>>();

    mark_required_state_calls(&context, &mut calls);

    let mut plan = StateCallPlan::default();
    for call in calls {
        let lowering = state_call_lowering(&context, &call);
        let arguments = plan.arguments.insert_many(build_call_arguments(
            &context,
            &call.target_machine,
            &call.target_state,
            call.required,
            &call.raw_arguments,
        ));

        plan.calls.insert(StateCall {
            source_machine: call.source_machine,
            source_state: call.source_state,
            statement_index: call.statement_index,
            receiver: call.receiver,
            target_machine: call.target_machine,
            target_state: call.target_state,
            argument_count: arguments.len(),
            arguments,
            reachable: call.reachable,
            required: call.required,
            lowering,
            resolution: call.resolution,
        });
    }

    plan
}

fn state_call_lowering(
    context: &StateAnalysisContext,
    call: &CollectedStateCall,
) -> StateCallLowering {
    if call.target_machine.is_empty() {
        StateCallLowering::Unresolved
    } else if state_call_targets_leaf(context, call) {
        StateCallLowering::InlineLeaf
    } else if state_call_targets_branching_state(context, call) {
        StateCallLowering::InlineBranching
    } else {
        StateCallLowering::InlineExpansion
    }
}

fn state_call_targets_branching_state(
    context: &StateAnalysisContext,
    call: &CollectedStateCall,
) -> bool {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == call.target_machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == call.target_state))
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_some_and(|transitions| !transitions.is_empty())
}

fn state_call_targets_leaf(context: &StateAnalysisContext, call: &CollectedStateCall) -> bool {
    let Some(machine) = context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == call.target_machine)
        .map(|(_, machine)| machine)
    else {
        return false;
    };
    let Some(state) = context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == call.target_state))
    else {
        return false;
    };

    let transitions_are_empty = context
        .control_flow
        .transitions
        .span(state.transitions)
        .is_none_or(|transitions| transitions.is_empty());
    if !transitions_are_empty {
        return false;
    }

    context
        .control_flow
        .operations
        .span(state.operations)
        .is_none_or(|operations| {
            operations.iter().all(|operation| {
                !matches!(operation.kind, OperationKind::Call { .. })
                    || context.state_statement_has_host_call(
                        &call.target_machine,
                        &call.target_state,
                        operation.statement_index,
                    )
            })
        })
}

fn collect_machine_state_calls(
    context: &StateAnalysisContext,
    machine: &MachineFlow,
) -> Vec<CollectedStateCall> {
    let mut calls = Vec::new();

    let Some(states) = context.control_flow.states.span(machine.states) else {
        return calls;
    };

    for state in states {
        let Some(operations) = context.control_flow.operations.span(state.operations) else {
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

            if context.state_statement_has_host_call(
                &machine.name,
                &state.name,
                operation.statement_index,
            ) {
                continue;
            }

            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                receiver.as_deref(),
                target,
            );

            calls.push(CollectedStateCall {
                source_machine: machine.name.to_string(),
                source_state: state.name.to_string(),
                statement_index: operation.statement_index,
                receiver: receiver
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "self".to_owned()),
                target_machine: resolved_target
                    .as_ref()
                    .map(|target| target.machine.clone())
                    .unwrap_or_default(),
                target_state: target.to_string(),
                raw_arguments: arguments.clone(),
                reachable: context.runtime_state_is_reachable(&machine.name, &state.name),
                required: false,
                resolution: resolved_target
                    .map(|target| target.resolution)
                    .unwrap_or(StateCallResolution::Unresolved),
            });
        }
    }

    calls
}

fn build_call_arguments<'a>(
    context: &StateAnalysisContext,
    target_machine: &str,
    target_state: &str,
    required: bool,
    raw_arguments: &'a [Expression],
) -> impl Iterator<Item = StateCallArgument> + 'a {
    let parameter_names = state_parameter_names(context, target_machine, target_state);

    raw_arguments
        .iter()
        .enumerate()
        .map(move |(index, expression)| StateCallArgument {
            index,
            parameter_name: parameter_names.get(index).cloned().unwrap_or_default(),
            expression: expression.clone(),
            kind: if matches!(expression, Expression::Mutable(_)) {
                StateCallArgumentKind::MutableAlias
            } else {
                StateCallArgumentKind::Value
            },
            required,
        })
}

fn state_parameter_names(
    context: &StateAnalysisContext,
    target_machine: &str,
    target_state: &str,
) -> Vec<String> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == target_machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == target_state))
        .map(|state| state.parameters.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

fn mark_required_state_calls(context: &StateAnalysisContext, calls: &mut [CollectedStateCall]) {
    let mut required_states = context
        .runtime_flow
        .states
        .iter()
        .map(|(_, state)| (state.machine.to_string(), state.state.to_string()))
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

            changed |= push_required_state(
                &mut required_states,
                call.target_machine.clone(),
                call.target_state.clone(),
            );
        }

        let states_snapshot = required_states.clone();
        for (machine_name, state_name) in states_snapshot {
            for target in transition_targets_from(context, &machine_name, &state_name) {
                if let RuntimeTransitionTarget::State { machine, state } = target {
                    changed |= push_required_state(
                        &mut required_states,
                        machine.to_string(),
                        state.to_string(),
                    );
                }
            }
        }
    }

    for call in calls {
        call.required = required_states
            .iter()
            .any(|(machine, state)| machine == &call.source_machine && state == &call.source_state);
    }
}

fn transition_targets_from(
    context: &StateAnalysisContext,
    machine_name: &str,
    state_name: &str,
) -> Vec<RuntimeTransitionTarget> {
    let Some(machine) = context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
    else {
        return Vec::new();
    };
    let Some(state) = context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
    else {
        return Vec::new();
    };
    let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
        return Vec::new();
    };

    transitions
        .iter()
        .flat_map(|transition| {
            let mut targets = vec![runtime_transition_target(
                machine,
                state_name,
                &transition.target,
            )];
            if let Some(continuation) = &transition.continuation {
                targets.push(runtime_transition_target(machine, state_name, continuation));
            }
            targets
        })
        .collect()
}

fn runtime_transition_target(
    machine: &MachineFlow,
    current_state: &str,
    target: &crate::control_flow::PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        crate::control_flow::PlannedTransitionTarget::State { name, .. } => {
            RuntimeTransitionTarget::State {
                machine: machine.name.clone(),
                state: name.clone(),
            }
        }
        crate::control_flow::PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => machine
            .contains
            .iter()
            .find(|contained| contained.name == *receiver)
            .map(|contained| RuntimeTransitionTarget::State {
                machine: contained.type_name.clone(),
                state: state.clone(),
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        crate::control_flow::PlannedTransitionTarget::SelfTarget => {
            RuntimeTransitionTarget::State {
                machine: machine.name.clone(),
                state: current_state.to_owned().into(),
            }
        }
        crate::control_flow::PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}

fn push_required_state(
    required_states: &mut Vec<(String, String)>,
    machine: String,
    state: String,
) -> bool {
    if required_states
        .iter()
        .any(|(required_machine, required_state)| {
            required_machine == &machine && required_state == &state
        })
    {
        false
    } else {
        required_states.push((machine, state));
        true
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
                machine: machine.name.to_string(),
                resolution: StateCallResolution::Local,
            });
        }

        return None;
    };

    if receiver == "self" && machine_has_state(control_flow, &machine.name, target_state) {
        return Some(ResolvedStateCall {
            machine: machine.name.to_string(),
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
                machine: contained.type_name.to_string(),
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
