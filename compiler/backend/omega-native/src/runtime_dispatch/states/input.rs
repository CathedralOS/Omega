use crate::control_flow::StateKey;
use crate::runtime_flow::{RuntimeFlowPlan, RuntimeState};
use omega_core::arena::Handle;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeStateInput {
    pub(super) handle: Handle<RuntimeState>,
    pub(super) key: StateKey,
    pub(super) machine: ProgramName,
    pub(super) state: ProgramName,
}

pub fn runtime_state_inputs(runtime_flow: &RuntimeFlowPlan) -> Vec<RuntimeStateInput> {
    runtime_flow
        .states
        .iter()
        .map(|(handle, runtime_state)| RuntimeStateInput {
            handle,
            key: runtime_state.key,
            machine: runtime_state.machine.clone(),
            state: runtime_state.state.clone(),
        })
        .collect()
}
