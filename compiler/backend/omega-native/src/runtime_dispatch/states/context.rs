use crate::control_flow::StateKey;
use crate::runtime_flow::{RuntimeEdge, RuntimeFlowPlan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchContext {
    pub(super) edges: Vec<RuntimeEdge>,
    pub(super) targets: Vec<StateDispatchTarget>,
}

impl StateDispatchContext {
    pub fn from_runtime_flow(runtime_flow: &RuntimeFlowPlan) -> Self {
        Self {
            edges: runtime_flow
                .edges
                .iter()
                .map(|(_, edge)| edge.clone())
                .collect(),
            targets: runtime_flow
                .states
                .iter()
                .map(|(handle, state)| StateDispatchTarget {
                    key: state.key,
                    dispatch_index: handle.arena_index(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StateDispatchTarget {
    pub(super) key: StateKey,
    pub(super) dispatch_index: u32,
}
