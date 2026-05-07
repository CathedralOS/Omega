use crate::ir::expression::{BinaryOperator, Expression};
use crate::ir::statement::TransitionGuard;
use crate::native::control_flow::{
    MachineFlow, OperationKind, PlannedTransitionTarget, StateFlow, TransitionFlow,
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
    let mut values = Vec::<(String, String)>::new();

    append_state_chain(
        native_plan,
        &native_plan.entry_machine,
        &native_plan.entry_state,
        &mut schedule,
        &mut visited,
        &mut values,
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
    values: &mut Vec<(String, String)>,
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
        apply_static_operations(native_plan, state, values);

        let transitions = native_plan
            .control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[]);

        match transitions {
            [] => return Ok(()),
            transitions => {
                let Some(transition) = select_transition(transitions, values, &current)? else {
                    return Ok(());
                };
                let Some(next_state) = next_state(
                    native_plan,
                    &current.machine,
                    machine,
                    state,
                    transition,
                    schedule,
                    visited,
                    values,
                )?
                else {
                    return Ok(());
                };

                current_machine_name = next_state.machine;
                current_state_name = next_state.state;
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
    values: &mut Vec<(String, String)>,
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
                values,
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

fn apply_static_operations(
    native_plan: &NativePlan,
    state: &StateFlow,
    values: &mut Vec<(String, String)>,
) {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return;
    };

    for operation in operations {
        let OperationKind::StaticAssignment { target, value } = &operation.kind else {
            continue;
        };

        if let Some((_, existing_value)) = values
            .iter_mut()
            .find(|(existing_target, _)| existing_target == target)
        {
            *existing_value = value.clone();
        } else {
            values.push((target.clone(), value.clone()));
        }
    }
}

fn select_transition<'plan>(
    transitions: &'plan [TransitionFlow],
    values: &[(String, String)],
    current: &ScheduledState,
) -> Result<Option<&'plan TransitionFlow>, String> {
    for transition in transitions {
        match guard_matches(&transition.guard, values) {
            Some(true) => return Ok(Some(transition)),
            Some(false) => continue,
            None => {
                return Err(format!(
                    "{}.{} has a guard native emission cannot evaluate statically yet",
                    current.machine, current.state
                ));
            }
        }
    }

    Err(format!(
        "{}.{} has no transition whose guard is satisfied",
        current.machine, current.state
    ))
}

fn guard_matches(guard: &TransitionGuard, values: &[(String, String)]) -> Option<bool> {
    match guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(expression) => evaluate_boolean(expression, values),
    }
}

fn evaluate_boolean(expression: &Expression, values: &[(String, String)]) -> Option<bool> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    match binary.operator {
        BinaryOperator::Equal => Some(
            resolve_static_value(&binary.left, values)?
                == resolve_static_value(&binary.right, values)?,
        ),
        BinaryOperator::NotEqual => Some(
            resolve_static_value(&binary.left, values)?
                != resolve_static_value(&binary.right, values)?,
        ),
        _ => None,
    }
}

fn resolve_static_value(expression: &Expression, values: &[(String, String)]) -> Option<String> {
    match expression {
        Expression::Name(_) => {
            let name = expression.display_name();
            values
                .iter()
                .find(|(target, _)| target == &name)
                .map(|(_, value)| value.clone())
                .or(Some(name))
        }
        Expression::Boolean(value) => Some(value.to_string()),
        Expression::Integer(value) => Some(value.to_string()),
        Expression::String(value) => Some(value.clone()),
        _ => None,
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
