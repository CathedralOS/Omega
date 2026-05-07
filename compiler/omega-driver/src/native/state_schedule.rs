use crate::native::control_flow::{PlannedTransitionTarget, StateFlow};
use crate::native::plan::NativePlan;
use crate::ir::statement::TransitionGuard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledState {
    pub machine: String,
    pub state: String,
}

pub fn build_entry_state_schedule(native_plan: &NativePlan) -> Result<Vec<ScheduledState>, String> {
    let (_, machine) = native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == native_plan.entry_machine)
        .ok_or_else(|| {
            format!(
                "entry machine `{}` was not present in the control-flow plan",
                native_plan.entry_machine
            )
        })?;
    let states = native_plan
        .control_flow
        .states
        .span(machine.states)
        .ok_or_else(|| {
            format!(
                "entry machine `{}` has an invalid state span",
                native_plan.entry_machine
            )
        })?;
    let mut current_index = states
        .iter()
        .position(|state| state.name == native_plan.entry_state)
        .ok_or_else(|| {
            format!(
                "entry state {}.{} was not present in the control-flow plan",
                native_plan.entry_machine, native_plan.entry_state
            )
        })?;
    let mut visited = Vec::<usize>::new();
    let mut schedule = Vec::new();

    loop {
        if visited.contains(&current_index) {
            return Err(format!(
                "{}.{} reaches a cycle; native emission does not support loops yet",
                native_plan.entry_machine, states[current_index].name
            ));
        }

        visited.push(current_index);

        let state = &states[current_index];
        schedule.push(ScheduledState {
            machine: native_plan.entry_machine.clone(),
            state: state.name.clone(),
        });

        let transitions = native_plan
            .control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[]);

        match transitions {
            [] => return Ok(schedule),
            [transition] if transition.guard == TransitionGuard::Always => {
                let Some(next_index) = next_state_index(state, &transition.target)? else {
                    return Ok(schedule);
                };

                if next_index >= states.len() {
                    return Err(format!(
                        "{}.{} transitions to invalid state index {}",
                        native_plan.entry_machine, state.name, next_index
                    ));
                }

                current_index = next_index;
            }
            [_] => {
                return Err(format!(
                    "{}.{} has a guarded transition; native emission supports only unconditional state chains so far",
                    native_plan.entry_machine, state.name
                ));
            }
            _ => {
                return Err(format!(
                    "{}.{} has {} transition(s); native emission supports only single-transition state chains so far",
                    native_plan.entry_machine,
                    state.name,
                    transitions.len()
                ));
            }
        }
    }
}

pub fn scheduled_state_contains(
    schedule: &[ScheduledState],
    machine_name: &str,
    state_name: &str,
) -> bool {
    schedule
        .iter()
        .any(|state| state.machine == machine_name && state.state == state_name)
}

fn next_state_index(
    state: &StateFlow,
    target: &PlannedTransitionTarget,
) -> Result<Option<usize>, String> {
    match target {
        PlannedTransitionTarget::State { index, .. } => Ok(Some(*index)),
        PlannedTransitionTarget::Terminal => Ok(None),
        PlannedTransitionTarget::SelfTarget => Err(format!(
            "{} self-transitions; native emission does not support loops yet",
            state.name
        )),
        PlannedTransitionTarget::Nested {
            receiver,
            state: nested_state,
        } => Err(format!(
            "{} transitions into nested machine `{receiver}.{nested_state}`; native emission does not support nested calls yet",
            state.name
        )),
    }
}
