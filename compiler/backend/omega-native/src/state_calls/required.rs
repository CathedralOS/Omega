use crate::control_flow::{MachineFlow, PlannedTransitionTarget, StateKey};
use crate::runtime_flow::RuntimeTransitionTarget;
use crate::state_analysis::StateAnalysisContext;

use super::collection::CollectedStateCall;
use super::lookups::state_key_is_valid;

pub(in crate::state_calls) fn mark_required_state_calls(
    context: &StateAnalysisContext,
    calls: &mut [CollectedStateCall],
) {
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
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::State { key, .. } => RuntimeTransitionTarget::State { key: *key },
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
            ..
        } => machine
            .contains
            .iter()
            .find(|contained| {
                if receiver_symbol.is_valid() {
                    contained.symbol == *receiver_symbol
                } else {
                    contained.name == *receiver
                }
            })
            .and_then(|contained| {
                context
                    .control_flow
                    .machine_by_symbol(contained.type_symbol)
                    .map(|machine| (contained, machine))
            })
            .and_then(|(_, target_machine)| {
                context
                    .control_flow
                    .states
                    .span(target_machine.states)
                    .and_then(|states| {
                        states.iter().find(|candidate| {
                            if state_symbol.is_valid() {
                                candidate.key.state == *state_symbol
                            } else {
                                candidate.name == *state
                            }
                        })
                    })
                    .map(|target_state| RuntimeTransitionTarget::State {
                        key: target_state.key,
                    })
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        PlannedTransitionTarget::SelfTarget => {
            RuntimeTransitionTarget::State { key: current_state }
        }
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
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
