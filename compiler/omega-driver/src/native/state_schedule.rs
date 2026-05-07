use crate::ir::statement::TransitionGuard;
use crate::native::control_flow::{
    MachineFlow, PlannedTransitionTarget, StateFlow, TransitionFlow,
};
use crate::native::plan::NativePlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledState {
    pub machine: String,
    pub state: String,
}

pub fn build_entry_state_schedule(native_plan: &NativePlan) -> Result<Vec<ScheduledState>, String> {
    let mut schedule = Vec::new();
    let mut visited = Vec::<ScheduledState>::new();

    append_state_chain(
        native_plan,
        &native_plan.entry_machine,
        &native_plan.entry_state,
        &mut schedule,
        &mut visited,
    )?;

    Ok(schedule)
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

fn append_state_chain(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
) -> Result<(), String> {
    let mut current_machine_name = machine_name.to_owned();
    let mut current_state_name = state_name.to_owned();

    loop {
        let current = ScheduledState {
            machine: current_machine_name.clone(),
            state: current_state_name.clone(),
        };

        if visited.contains(&current) {
            return Err(format!(
                "{}.{} reaches a cycle; native emission does not support loops yet",
                current.machine, current.state
            ));
        }

        visited.push(current.clone());
        schedule.push(current.clone());

        let machine = machine_flow(native_plan, &current.machine)?;
        let state = state_flow(native_plan, machine, &current.state)?;

        let transitions = native_plan
            .control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[]);

        match transitions {
            [] => return Ok(()),
            [transition] if transition.guard == TransitionGuard::Always => {
                let Some(next_state) = next_state(
                    native_plan,
                    &current.machine,
                    machine,
                    state,
                    transition,
                    schedule,
                    visited,
                )?
                else {
                    return Ok(());
                };

                current_machine_name = next_state.machine;
                current_state_name = next_state.state;
            }
            [_] => {
                return Err(format!(
                    "{}.{} has a guarded transition; native emission supports only unconditional state chains so far",
                    current.machine, current.state
                ));
            }
            _ => {
                return Err(format!(
                    "{}.{} has {} transition(s); native emission supports only single-transition state chains so far",
                    current.machine,
                    current.state,
                    transitions.len()
                ));
            }
        }
    }
}

fn next_state(
    native_plan: &NativePlan,
    machine_name: &str,
    machine: &MachineFlow,
    state: &StateFlow,
    transition: &TransitionFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
) -> Result<Option<ScheduledState>, String> {
    match &transition.target {
        PlannedTransitionTarget::State { index, name } => {
            validate_state_index(native_plan, machine, *index, machine_name, &state.name)?;
            Ok(Some(ScheduledState {
                machine: machine_name.to_owned(),
                state: name.clone(),
            }))
        }
        PlannedTransitionTarget::Terminal => Ok(None),
        PlannedTransitionTarget::SelfTarget => Err(format!(
            "{} self-transitions; native emission does not support loops yet",
            state.name
        )),
        PlannedTransitionTarget::Nested {
            receiver,
            state: nested_state,
        } => {
            let nested_machine_name = machine
                .contains
                .iter()
                .find(|contained| contained.name == *receiver)
                .map(|contained| contained.type_name.as_str())
                .ok_or_else(|| {
                    format!(
                        "{}.{} transitions into unknown nested machine `{receiver}`",
                        machine_name, state.name
                    )
                })?;

            append_state_chain(
                native_plan,
                nested_machine_name,
                nested_state,
                schedule,
                visited,
            )?;

            match &transition.continuation {
                Some(PlannedTransitionTarget::State { index, name }) => {
                    validate_state_index(native_plan, machine, *index, machine_name, &state.name)?;
                    Ok(Some(ScheduledState {
                        machine: machine_name.to_owned(),
                        state: name.clone(),
                    }))
                }
                Some(PlannedTransitionTarget::Terminal) | None => Ok(None),
                Some(PlannedTransitionTarget::SelfTarget) => Err(format!(
                    "{}.{} nested continuation self-transitions; native emission does not support loops yet",
                    machine_name, state.name
                )),
                Some(PlannedTransitionTarget::Nested {
                    receiver,
                    state: nested_state,
                }) => Err(format!(
                    "{}.{} nested continuation targets `{receiver}.{nested_state}`; native emission supports one nested call at a time so far",
                    machine_name, state.name
                )),
            }
        }
    }
}

fn machine_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
) -> Result<&'plan MachineFlow, String> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
        .ok_or_else(|| format!("machine `{machine_name}` was not present in the control-flow plan"))
}

fn state_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine: &MachineFlow,
    state_name: &str,
) -> Result<&'plan StateFlow, String> {
    native_plan
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .ok_or_else(|| {
            format!(
                "state {}.{} was not present in the control-flow plan",
                machine.name, state_name
            )
        })
}

fn validate_state_index(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    state_index: usize,
    source_machine: &str,
    source_state: &str,
) -> Result<(), String> {
    let states = native_plan
        .control_flow
        .states
        .span(machine.states)
        .ok_or_else(|| format!("machine `{}` has an invalid state span", machine.name))?;

    if state_index >= states.len() {
        return Err(format!(
            "{}.{} transitions to invalid state index {}",
            source_machine, source_state, state_index
        ));
    }

    Ok(())
}
