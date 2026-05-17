use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_graph::{RuntimeEdge, RuntimeFlowPlan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchContext {
    pub(super) edges: Arena<RuntimeEdge>,
    pub(super) targets: Arena<StateDispatchTarget>,
}

impl StateDispatchContext {
    pub fn from_runtime_flow(runtime_flow: &RuntimeFlowPlan) -> Self {
        let mut edges = Arena::with_capacity(runtime_flow.edges.len());
        edges.insert_many(runtime_flow.edges.iter().map(|(_, edge)| edge.clone()));
        let mut targets = Arena::with_capacity(runtime_flow.states.len());
        targets.insert_many(runtime_flow.states.iter().map(|(handle, state)| {
            StateDispatchTarget {
                key: state.key,
                dispatch_index: handle.arena_index(),
            }
        }));

        Self { edges, targets }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StateDispatchTarget {
    pub(super) key: StateKey,
    pub(super) dispatch_index: u32,
}
