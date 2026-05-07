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
    let mut aliases = Vec::<(String, String)>::new();

    append_state_chain(
        native_plan,
        &native_plan.entry_machine,
        &native_plan.entry_state,
        &mut schedule,
        &mut visited,
        &mut values,
        &mut aliases,
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
    aliases: &mut Vec<(String, String)>,
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
                "cycle {}; native emission does not support loops yet",
                cycle_path(&visited, &current)
            ));
        }

        visited.push(current.clone());
        schedule.push(current.clone());

        let machine = machine_flow(native_plan, &current.machine)?;
        let state = state_flow(native_plan, machine, &current.state)?;
        append_local_state_calls(
            native_plan,
            &current,
            machine,
            state,
            schedule,
            visited,
            values,
            aliases,
        )?;
        apply_static_operations(native_plan, state, aliases, values);

        let transitions = native_plan
            .control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[]);

        match transitions {
            [] => return Ok(()),
            transitions => {
                let Some(transition) = select_transition(transitions, values, aliases, &current)?
                else {
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
                    aliases,
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

fn cycle_path(visited: &[ScheduledState], current: &ScheduledState) -> String {
    let start = visited
        .iter()
        .position(|state| state == current)
        .unwrap_or(0);
    visited[start..]
        .iter()
        .chain(std::iter::once(current))
        .map(|state| format!("{}.{}", state.machine, state.state))
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn append_local_state_calls(
    native_plan: &NativePlan,
    current: &ScheduledState,
    machine: &MachineFlow,
    state: &StateFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(String, String)>,
    aliases: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Err(format!(
            "{}.{} has an invalid operation span",
            current.machine, current.state
        ));
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

        let is_platform_call = native_plan.host_calls.calls.iter().any(|(_, host_call)| {
            host_call.machine == current.machine
                && host_call.state == current.state
                && host_call.statement_index == operation.statement_index
        }) || native_plan.host_calls.unsupported_calls.iter().any(
            |(_, host_call)| {
                host_call.machine == current.machine
                    && host_call.state == current.state
                    && host_call.statement_index == operation.statement_index
            },
        );

        if is_platform_call {
            continue;
        }

        let target_machine = resolve_state_call_machine(native_plan, machine, receiver.as_deref())
            .ok_or_else(|| {
                format!(
                    "{}.{} statement {} calls unknown state receiver `{}`",
                    current.machine,
                    current.state,
                    operation.statement_index,
                    receiver.as_deref().unwrap_or("self")
                )
            })?;

        let saved_alias_count = aliases.len();
        let saved_visited_count = visited.len();
        bind_state_arguments(
            native_plan,
            &target_machine,
            target,
            arguments,
            aliases,
            values,
        )?;

        append_state_chain(
            native_plan,
            &target_machine,
            target,
            schedule,
            visited,
            values,
            aliases,
        )?;
        visited.truncate(saved_visited_count);
        aliases.truncate(saved_alias_count);
    }

    Ok(())
}

fn resolve_state_call_machine(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    receiver: Option<&str>,
) -> Option<String> {
    let Some(receiver) = receiver else {
        return Some(machine.name.clone());
    };

    machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
        .map(|contained| contained.type_name.clone())
        .or_else(|| {
            native_plan
                .control_flow
                .machines
                .iter()
                .find(|(_, candidate)| candidate.name == receiver)
                .map(|(_, candidate)| candidate.name.clone())
        })
}

fn bind_state_arguments(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    arguments: &[Expression],
    aliases: &mut Vec<(String, String)>,
    values: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let machine = machine_flow(native_plan, machine_name)?;
    let state = state_flow(native_plan, machine, state_name)?;

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument = argument_binding_place_name(argument, aliases);
        if let Some(canonical_argument) = canonical_argument {
            set_alias(aliases, parameter.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(argument, aliases, values) {
            set_static_value(values, parameter.clone(), value);
        }
    }

    Ok(())
}

fn set_alias(aliases: &mut Vec<(String, String)>, parameter: String, target: String) {
    if let Some((_, existing_target)) = aliases
        .iter_mut()
        .find(|(existing_parameter, _)| existing_parameter == &parameter)
    {
        *existing_target = target;
    } else {
        aliases.push((parameter, target));
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
    aliases: &mut Vec<(String, String)>,
) -> Result<Option<ScheduledState>, String> {
    match &transition.target {
        PlannedTransitionTarget::State {
            index,
            name,
            arguments,
        } => {
            validate_state_index(native_plan, machine, *index, machine_name, &state.name)?;
            bind_state_arguments(native_plan, machine_name, name, arguments, aliases, values)?;
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
            arguments,
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

            let saved_alias_count = aliases.len();
            let saved_visited_count = visited.len();
            bind_state_arguments(
                native_plan,
                nested_machine_name,
                nested_state,
                arguments,
                aliases,
                values,
            )?;
            append_state_chain(
                native_plan,
                nested_machine_name,
                nested_state,
                schedule,
                visited,
                values,
                aliases,
            )?;
            visited.truncate(saved_visited_count);
            aliases.truncate(saved_alias_count);

            match &transition.continuation {
                Some(PlannedTransitionTarget::State {
                    index,
                    name,
                    arguments,
                }) => {
                    validate_state_index(native_plan, machine, *index, machine_name, &state.name)?;
                    bind_state_arguments(native_plan, machine_name, name, arguments, aliases, values)?;
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
                    ..
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
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return;
    };

    for operation in operations {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                apply_static_assignment(target, value, aliases, values);
            }
            _ => {}
        };
    }
}

fn apply_static_assignment(
    target: &Expression,
    value: &Expression,
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(target_name) = shallow_canonical_place_name(target, aliases) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target = format!("{target_name}::{}", field.name);
            if let Some(source_name) = canonical_place_name(&field.value, aliases) {
                copy_static_prefix(values, &source_name, &field_target);
            }
            if let Some(field_value) = resolve_static_value(&field.value, aliases, values) {
                set_static_value(values, field_target, field_value);
            }
        }
        return;
    }

    if let Some(source_name) = canonical_place_name(value, aliases) {
        copy_static_prefix(values, &source_name, &target_name);
    }

    let Some(value) = resolve_static_value(value, aliases, values) else {
        return;
    };

    set_static_value(values, target_name, value);
}

fn set_static_value(values: &mut Vec<(String, String)>, target: String, value: String) {
    if let Some((_, existing_value)) = values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        values.push((target, value));
    }
}

fn copy_static_prefix(values: &mut Vec<(String, String)>, source_name: &str, target_name: &str) {
    let source_prefix = format!("{source_name}::");
    let copied_values = values
        .iter()
        .filter_map(|(existing_name, value)| {
            existing_name
                .strip_prefix(&source_prefix)
                .map(|suffix| (format!("{target_name}::{suffix}"), value.clone()))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(values, copied_name, copied_value);
    }
}

fn select_transition<'plan>(
    transitions: &'plan [TransitionFlow],
    values: &[(String, String)],
    aliases: &[(String, String)],
    current: &ScheduledState,
) -> Result<Option<&'plan TransitionFlow>, String> {
    for transition in transitions {
        match guard_matches(&transition.guard, aliases, values) {
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

fn guard_matches(
    guard: &TransitionGuard,
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<bool> {
    match guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(expression) => evaluate_boolean(expression, aliases, values),
    }
}

fn evaluate_boolean(
    expression: &Expression,
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<bool> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    match binary.operator {
        BinaryOperator::Equal => Some(
            resolve_static_value(&binary.left, aliases, values)?
                == resolve_static_value(&binary.right, aliases, values)?,
        ),
        BinaryOperator::NotEqual => Some(
            resolve_static_value(&binary.left, aliases, values)?
                != resolve_static_value(&binary.right, aliases, values)?,
        ),
        _ => None,
    }
}

fn resolve_static_value(
    expression: &Expression,
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<String> {
    match expression {
        Expression::Mutable(inner_expression) => resolve_static_value(inner_expression, aliases, values),
        Expression::Name(_) | Expression::Indexed(_) => {
            let name = canonical_place_name(expression, aliases)?;
            values
                .iter()
                .find(|(target, _)| target == &name)
                .map(|(_, value)| value.clone())
                .or_else(|| static_symbol_name(expression))
        }
        Expression::Boolean(value) => Some(value.to_string()),
        Expression::Integer(value) => Some(value.to_string()),
        Expression::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn canonical_place_name(expression: &Expression, aliases: &[(String, String)]) -> Option<String> {
    let name = match expression {
        Expression::Mutable(inner_expression) => {
            return canonical_place_name(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => expression.display_name(),
        _ => return None,
    };

    Some(resolve_alias(&name, aliases))
}

fn argument_binding_place_name(
    expression: &Expression,
    aliases: &[(String, String)],
) -> Option<String> {
    match expression {
        Expression::Mutable(inner_expression) => shallow_canonical_place_name(inner_expression, aliases),
        _ => canonical_place_name(expression, aliases),
    }
}

fn shallow_canonical_place_name(
    expression: &Expression,
    aliases: &[(String, String)],
) -> Option<String> {
    let name = match expression {
        Expression::Mutable(inner_expression) => {
            return shallow_canonical_place_name(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => expression.display_name(),
        _ => return None,
    };

    Some(resolve_alias_once(&name, aliases))
}

fn resolve_alias(name: &str, aliases: &[(String, String)]) -> String {
    let mut resolved = name.to_owned();

    for _ in 0..aliases.len() {
        let Some((alias, target)) = aliases
            .iter()
            .rev()
            .find(|(alias, _)| alias_applies(&resolved, alias))
        else {
            return resolved;
        };

        resolved = replace_alias_prefix(&resolved, alias, target);
    }

    resolved
}

fn resolve_alias_once(name: &str, aliases: &[(String, String)]) -> String {
    aliases
        .iter()
        .rev()
        .find(|(alias, _)| alias_applies(name, alias))
        .map_or_else(|| name.to_owned(), |(alias, target)| replace_alias_prefix(name, alias, target))
}

fn alias_applies(name: &str, alias: &str) -> bool {
    name == alias || name.starts_with(&format!("{alias}::")) || name.starts_with(&format!("{alias}["))
}

fn replace_alias_prefix(name: &str, alias: &str, target: &str) -> String {
    if name == alias {
        return target.to_owned();
    }

    if let Some(suffix) = name.strip_prefix(&format!("{alias}::")) {
        return format!("{target}::{suffix}");
    }

    if let Some(suffix) = name.strip_prefix(alias) {
        return format!("{target}{suffix}");
    }

    name.to_owned()
}

fn static_symbol_name(expression: &Expression) -> Option<String> {
    let Expression::Name(path) = expression else {
        return None;
    };

    if path
        .first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
    {
        Some(expression.display_name())
    } else {
        None
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
