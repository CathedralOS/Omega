use crate::StateCallPlanningContext;
use omega_control_flow::{StateFlow, StateKey};

pub(crate) fn state_flow_from_key(
    context: &StateCallPlanningContext,
    state_key: StateKey,
) -> Option<&StateFlow> {
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

pub(crate) fn state_key_is_valid(state_key: StateKey) -> bool {
    state_key.machine.is_valid() && state_key.state.is_valid()
}
