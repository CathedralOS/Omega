use crate::control_flow::{MachineFlow, StateFlow, StateKey};
use crate::plan::NativePlan;
use omega_core::symbols::SymbolHandle;

pub(super) fn machine_flow<'plan>(
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

pub(super) fn machine_flow_by_symbol(
    native_plan: &NativePlan,
    machine_symbol: SymbolHandle,
) -> Result<&MachineFlow, String> {
    native_plan
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

pub(super) fn state_flow_by_key(
    native_plan: &NativePlan,
    key: StateKey,
) -> Result<&StateFlow, String> {
    let machine = machine_flow_by_symbol(native_plan, key.machine)?;

    native_plan
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
