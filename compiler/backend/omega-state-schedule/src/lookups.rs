use crate::StateScheduleContext;
use omega_control_flow::{MachineFlow, StateFlow, StateKey};
use omega_core::symbols::SymbolHandle;

pub(super) fn machine_flow<'plan>(
    context: &'plan StateScheduleContext,
    machine_name: &str,
) -> Result<&'plan MachineFlow, String> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .map(|(_, machine)| machine)
        .ok_or_else(|| format!("machine `{machine_name}` was not present in the control-flow plan"))
}

pub(super) fn machine_flow_by_symbol(
    context: &StateScheduleContext,
    machine_symbol: SymbolHandle,
) -> Result<&MachineFlow, String> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == machine_symbol)
        .map(|(_, machine)| machine)
        .ok_or_else(|| {
            format!(
                "machine symbol {} was not present in the control-flow plan",
                machine_symbol.arena_index()
            )
        })
}

pub(super) fn state_flow<'plan>(
    context: &'plan StateScheduleContext,
    machine: &MachineFlow,
    state_name: &str,
) -> Result<&'plan StateFlow, String> {
    context
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

pub(super) fn state_flow_by_key(
    context: &StateScheduleContext,
    key: StateKey,
) -> Result<&StateFlow, String> {
    let machine = machine_flow_by_symbol(context, key.machine)?;

    context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| states.iter().find(|state| state.key == key))
        .ok_or_else(|| {
            format!(
                "state key {}.{}#{} was not present in the control-flow plan",
                key.machine.arena_index(),
                key.state.arena_index(),
                key.segment_index
            )
        })
}

pub(super) fn validate_state_index(
    context: &StateScheduleContext,
    machine: &MachineFlow,
    state_index: usize,
    source_machine: &str,
    source_state: &str,
) -> Result<(), String> {
    let states = context
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
