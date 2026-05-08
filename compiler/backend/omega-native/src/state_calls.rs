mod collection;
mod model;

use crate::control_flow::{MachineFlow, OperationKind, StateKey};
use crate::plan::NativePlan;
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_analysis::StateAnalysisContext;
use collection::{collect_machine_state_calls, CollectedStateCall};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use std::sync::Arc;

pub use model::{
    StateCall, StateCallArgument, StateCallArgumentKind, StateCallLowering, StateCallPlan,
    StateCallResolution,
};

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
            call.target_key,
            call.required,
            &call.raw_arguments,
        ));

        plan.calls.insert(StateCall {
            source_key: call.source_key,
            source_machine: call.source_machine,
            source_state: call.source_state,
            statement_index: call.statement_index,
            receiver: call.receiver,
            target_key: call.target_key,
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
    if !state_key_is_valid(call.target_key) {
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
    state_flow_from_key(context, call.target_key)
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_some_and(|transitions| !transitions.is_empty())
}

fn state_call_targets_leaf(context: &StateAnalysisContext, call: &CollectedStateCall) -> bool {
    let Some(state) = state_flow_from_key(context, call.target_key) else {
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
                    || context.state_statement_has_host_call_by_key(
                        call.target_key,
                        operation.statement_index,
                    )
            })
        })
}

fn build_call_arguments<'a>(
    context: &StateAnalysisContext,
    target_key: StateKey,
    required: bool,
    raw_arguments: &'a [Expression],
) -> impl Iterator<Item = StateCallArgument> + 'a {
    let parameter_names = state_parameter_names(context, target_key);

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

fn state_parameter_names(context: &StateAnalysisContext, target_key: StateKey) -> Vec<ProgramName> {
    state_flow_from_key(context, target_key)
        .map(|state| {
            state
                .parameters
                .iter()
                .map(|parameter| parameter.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn mark_required_state_calls(context: &StateAnalysisContext, calls: &mut [CollectedStateCall]) {
    let mut required_states = context
        .runtime_flow
        .states
        .iter()
        .map(|(_, state)| state.key)
        .collect::<Vec<_>>();
    let mut changed = true;

    while changed {
        changed = false;

        for call in calls.iter_mut() {
            let source_is_required = required_states.contains(&call.source_key);

            if !source_is_required {
                continue;
            }

            if !call.required {
                call.required = true;
                changed = true;
            }

            if !state_key_is_valid(call.target_key) {
                continue;
            }

            changed |= push_required_state(&mut required_states, call.target_key);
        }

        let states_snapshot = required_states.clone();
        for state_key in states_snapshot {
            for target in transition_targets_from(context, state_key) {
                if let RuntimeTransitionTarget::State { key, .. } = target {
                    changed |= push_required_state(&mut required_states, key);
                }
            }
        }
    }

    for call in calls {
        call.required = required_states
            .iter()
            .any(|required_key| *required_key == call.source_key);
    }
}

fn transition_targets_from(
    context: &StateAnalysisContext,
    state_key: StateKey,
) -> Vec<RuntimeTransitionTarget> {
    let Some(machine) = context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .map(|(_, machine)| machine)
    else {
        return Vec::new();
    };
    let Some(state) = context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.key == state_key))
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
                context,
                machine,
                state.key,
                &transition.target,
            )];
            if let Some(continuation) = &transition.continuation {
                targets.push(runtime_transition_target(
                    context,
                    machine,
                    state.key,
                    continuation,
                ));
            }
            targets
        })
        .collect()
}

fn runtime_transition_target(
    context: &StateAnalysisContext,
    machine: &MachineFlow,
    current_state: StateKey,
    target: &crate::control_flow::PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        crate::control_flow::PlannedTransitionTarget::State { key, name, .. } => {
            RuntimeTransitionTarget::State {
                key: *key,
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
            .and_then(|contained| {
                context
                    .control_flow
                    .machines
                    .iter()
                    .find(|(_, machine)| machine.symbol == contained.type_symbol)
                    .map(|(_, machine)| (contained, machine))
            })
            .and_then(|(contained, target_machine)| {
                context
                    .control_flow
                    .states
                    .span(target_machine.states)
                    .and_then(|states| states.iter().find(|candidate| candidate.name == *state))
                    .map(|target_state| RuntimeTransitionTarget::State {
                        key: target_state.key,
                        machine: contained.type_name.clone(),
                        state: target_state.name.clone(),
                    })
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        crate::control_flow::PlannedTransitionTarget::SelfTarget => {
            RuntimeTransitionTarget::State {
                key: current_state,
                machine: machine.name.clone(),
                state: context
                    .control_flow
                    .states
                    .span(machine.states)
                    .and_then(|states| states.iter().find(|state| state.key == current_state))
                    .map(|state| state.name.clone())
                    .unwrap_or_default(),
            }
        }
        crate::control_flow::PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}

fn push_required_state(required_states: &mut Vec<StateKey>, state_key: StateKey) -> bool {
    if required_states.contains(&state_key) {
        false
    } else {
        required_states.push(state_key);
        true
    }
}

fn state_flow_from_key(
    context: &StateAnalysisContext,
    state_key: StateKey,
) -> Option<&crate::control_flow::StateFlow> {
    let machine = context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .map(|(_, machine)| machine)?;

    context
        .control_flow
        .states
        .span(machine.states)?
        .iter()
        .find(|state| state.key == state_key)
}

fn state_key_is_valid(state_key: StateKey) -> bool {
    state_key.machine.is_valid() && state_key.state.is_valid()
}
