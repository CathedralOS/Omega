use crate::control_flow::{StateFlow, StateKey};
use crate::state_analysis::StateAnalysisContext;

pub(in crate::state_calls) fn state_flow_from_key(
    context: &StateAnalysisContext,
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

pub(in crate::state_calls) fn state_key_is_valid(state_key: StateKey) -> bool {
    state_key.machine.is_valid() && state_key.state.is_valid()
}
