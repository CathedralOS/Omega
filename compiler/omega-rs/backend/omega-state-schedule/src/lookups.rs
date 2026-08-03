use crate::StateScheduleContext;
use omega_control_flow::{MachineFlow, StateFlow, StateKey};
use psi_symbols::SymbolHandle;

pub(super) fn machine_flow_by_symbol<'plan>(
    context: &StateScheduleContext<'plan>,
    machine_symbol: SymbolHandle,
) -> Result<&'plan MachineFlow, String> {
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

pub(super) fn state_flow_by_key<'plan>(
    context: &StateScheduleContext<'plan>,
    key: StateKey,
) -> Result<&'plan StateFlow, String> {
    let machine = machine_flow_by_symbol(context, key.machine)?;

    // A scheduled state may be a SEGMENT (segment_index > 0) of a control-flow
    // state that the runtime flow split at its dispatched-call boundaries. The
    // control-flow plan only holds the unsplit state (segment 0), so match on
    // machine+state and ignore segment_index -- segments share the parent's flow.
    context
        .control_flow
        .states
        .span(machine.states)
        .and_then(|states| {
            states
                .iter()
                .find(|state| state.key.state == key.state && state.key.machine == key.machine)
        })
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
