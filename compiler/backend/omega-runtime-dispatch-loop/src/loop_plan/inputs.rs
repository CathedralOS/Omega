use omega_control_flow::StateKey;
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopCaseInput {
    pub(super) key: StateKey,
    pub(super) dispatch_index: u32,
    pub(super) label: String,
    pub(super) edges: Vec<DispatchEdge>,
}

pub fn runtime_dispatch_loop_inputs(
    state_dispatch: &StateDispatchPlan,
) -> Vec<RuntimeDispatchLoopCaseInput> {
    state_dispatch
        .states
        .iter()
        .map(|(_, state)| RuntimeDispatchLoopCaseInput {
            key: state.key,
            dispatch_index: state.dispatch_index,
            label: state.label.clone(),
            edges: state_dispatch
                .edges
                .span(state.edges)
                .unwrap_or(&[])
                .to_vec(),
        })
        .collect()
}

pub(crate) fn dispatch_index_for_key(state_dispatch: &StateDispatchPlan, key: StateKey) -> u32 {
    state_dispatch
        .states
        .iter()
        .find(|(_, dispatch_state)| dispatch_state.key == key)
        .map(|(_, dispatch_state)| dispatch_state.dispatch_index)
        .unwrap_or(0)
}
