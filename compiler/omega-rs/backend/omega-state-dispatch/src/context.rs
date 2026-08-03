use omega_control_flow::StateKey;
use omega_state_graph::{CallContext, RuntimeFlowPlan};
use psi_arena::Arena;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchContext {
    pub(super) runtime_flow: Arc<RuntimeFlowPlan>,
    pub(super) targets: Arena<StateDispatchTarget>,
}

impl StateDispatchContext {
    pub fn from_runtime_flow(runtime_flow: Arc<RuntimeFlowPlan>) -> Self {
        let mut targets = Arena::with_capacity(runtime_flow.states.len());
        targets.insert_many(runtime_flow.states.iter().map(|(handle, state)| {
            StateDispatchTarget {
                key: state.key,
                context: state.context,
                dispatch_index: handle.arena_index(),
            }
        }));

        Self {
            runtime_flow,
            targets,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StateDispatchTarget {
    pub(super) key: StateKey,
    /// The call-context clone this dispatch case represents. Distinguishes
    /// specialized copies of the same source state.
    pub(super) context: CallContext,
    pub(super) dispatch_index: u32,
}
