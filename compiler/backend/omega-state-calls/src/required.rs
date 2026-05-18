use crate::StateCallPlanningContext;
use omega_control_flow::{MachineFlow, PlannedTransitionTarget, StateKey};
use omega_state_graph::RuntimeTransitionTarget;
use std::collections::HashSet;

type StateKeyId = (u32, u32, usize);
const INLINE_REQUIRED_STATE_COUNT: usize = 8;

use super::collection::CollectedStateCall;
use super::lookups::state_key_is_valid;

pub(crate) fn mark_required_state_calls(
    context: &StateCallPlanningContext,
    calls: &mut [CollectedStateCall],
) {
    let required_state_capacity = context
        .runtime_flow
        .states
        .len()
        .saturating_add(calls.len());
    let mut required_states = RequiredStates::with_capacity(required_state_capacity);
    for (_, state) in context.runtime_flow.states.iter() {
        required_states.push(state.key);
    }

    let mut changed = true;

    while changed {
        changed = false;

        for call in calls.iter_mut() {
            if !required_states.contains(call.source_key) {
                continue;
            }

            if !call.required {
                call.required = true;
                changed = true;
            }

            if !state_key_is_valid(call.target_key) {
                continue;
            }

            changed |= required_states.push(call.target_key);
        }

        while let Some(state_key) = required_states.next_unprocessed() {
            for_each_transition_target_from(context, state_key, |target| {
                if let RuntimeTransitionTarget::State { key, .. } = *target {
                    changed |= required_states.push(key);
                }
            });
        }
    }

    for call in calls {
        call.required = required_states.contains(call.source_key);
    }
}

fn for_each_transition_target_from(
    context: &StateCallPlanningContext,
    state_key: StateKey,
    mut visit: impl FnMut(&RuntimeTransitionTarget),
) {
    let Some(machine) = context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .map(|(_, machine)| machine)
    else {
        return;
    };
    let Some(state) = context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.key == state_key))
    else {
        return;
    };
    let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
        return;
    };

    for transition in transitions {
        let target = runtime_transition_target(context, machine, state.key, &transition.target);
        visit(&target);
        if transition.continuation != PlannedTransitionTarget::None {
            let continuation =
                runtime_transition_target(context, machine, state.key, &transition.continuation);
            visit(&continuation);
        }
    }
}

fn runtime_transition_target(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
    current_state: StateKey,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::None => RuntimeTransitionTarget::None,
        PlannedTransitionTarget::State { key, .. } => RuntimeTransitionTarget::State { key: *key },
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            ..
        } => context
            .control_flow
            .machine_contains(machine)
            .iter()
            .find(|contained| receiver_symbol.is_valid() && contained.symbol == *receiver_symbol)
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
                            state_symbol.is_valid() && candidate.key.state == *state_symbol
                        })
                    })
                    .map(|target_state| RuntimeTransitionTarget::State {
                        key: target_state.key,
                    })
            })
            .unwrap_or(RuntimeTransitionTarget::Unknown),
        PlannedTransitionTarget::SelfTarget => {
            RuntimeTransitionTarget::State { key: current_state }
        }
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredStates {
    states: RequiredStateList,
    set: HashSet<StateKeyId>,
    next_unprocessed: usize,
}

impl RequiredStates {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            states: RequiredStateList::with_capacity(state_capacity),
            set: HashSet::with_capacity(state_capacity),
            next_unprocessed: 0,
        }
    }

    fn contains(&self, state_key: StateKey) -> bool {
        self.set.contains(&state_key_id(state_key))
    }

    fn push(&mut self, state_key: StateKey) -> bool {
        if self.set.insert(state_key_id(state_key)) {
            self.states.push(state_key);
            return true;
        }

        false
    }

    fn next_unprocessed(&mut self) -> Option<StateKey> {
        let state_key = self.states.get(self.next_unprocessed)?;
        self.next_unprocessed += 1;
        Some(state_key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequiredStateList {
    inline: [Option<StateKey>; INLINE_REQUIRED_STATE_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl RequiredStateList {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_REQUIRED_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(
                state_capacity.saturating_sub(INLINE_REQUIRED_STATE_COUNT),
            ),
        }
    }

    fn get(&self, index: usize) -> Option<StateKey> {
        if index >= self.len {
            return None;
        }

        if index < INLINE_REQUIRED_STATE_COUNT {
            return self.inline[index];
        }

        self.overflow
            .get(index - INLINE_REQUIRED_STATE_COUNT)
            .copied()
    }

    fn push(&mut self, state_key: StateKey) {
        if self.len < INLINE_REQUIRED_STATE_COUNT {
            self.inline[self.len] = Some(state_key);
        } else {
            self.overflow.push(state_key);
        }

        self.len += 1;
    }
}

fn state_key_id(state_key: StateKey) -> StateKeyId {
    (
        state_key.machine.arena_index(),
        state_key.state.arena_index(),
        state_key.segment_index,
    )
}
