use crate::runtime_flow::{RuntimeFlowPlan, RuntimeState};
use omega_control_flow::StateKey;
use omega_core::arena::Handle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStateInput {
    pub(super) handle: Handle<RuntimeState>,
    pub(super) key: StateKey,
}

pub fn runtime_state_inputs(runtime_flow: &RuntimeFlowPlan) -> Vec<RuntimeStateInput> {
    runtime_flow
        .states
        .iter()
        .map(|(handle, runtime_state)| RuntimeStateInput {
            handle,
            key: runtime_state.key,
        })
        .collect()
}
