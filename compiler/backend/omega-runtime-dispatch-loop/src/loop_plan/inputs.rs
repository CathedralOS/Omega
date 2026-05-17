use omega_control_flow::StateKey;
use omega_state_dispatch::StateDispatchPlan;

pub(crate) fn dispatch_index_for_key(state_dispatch: &StateDispatchPlan, key: StateKey) -> u32 {
    state_dispatch
        .states
        .iter()
        .find(|(_, dispatch_state)| dispatch_state.key == key)
        .map(|(_, dispatch_state)| dispatch_state.dispatch_index)
        .unwrap_or(0)
}
