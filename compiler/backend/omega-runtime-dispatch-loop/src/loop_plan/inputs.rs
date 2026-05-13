use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_dispatch::{DispatchEdge, StateDispatchPlan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopCaseInput {
    pub(super) key: StateKey,
    pub(super) dispatch_index: u32,
    pub(super) label: String,
    pub(super) edges: HandleSpan<DispatchEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopInputs {
    pub(super) cases: Arena<RuntimeDispatchLoopCaseInput>,
    pub(super) span: HandleSpan<RuntimeDispatchLoopCaseInput>,
}

impl RuntimeDispatchLoopInputs {
    pub fn len(&self) -> usize {
        self.span.len()
    }

    pub fn is_empty(&self) -> bool {
        self.span.is_empty()
    }

    pub(crate) fn get(&self, index: usize) -> Option<&RuntimeDispatchLoopCaseInput> {
        self.cases.span(self.span).and_then(|cases| cases.get(index))
    }
}

pub fn runtime_dispatch_loop_inputs(
    state_dispatch: &StateDispatchPlan,
) -> RuntimeDispatchLoopInputs {
    let mut inputs = RuntimeDispatchLoopInputs::default();
    inputs.span = inputs.cases.insert_many(state_dispatch.states.iter().map(|(_, state)| {
        RuntimeDispatchLoopCaseInput {
            key: state.key,
            dispatch_index: state.dispatch_index,
            label: state.label.clone(),
            edges: state.edges,
        }
    }));
    inputs
}

pub(crate) fn dispatch_index_for_key(state_dispatch: &StateDispatchPlan, key: StateKey) -> u32 {
    state_dispatch
        .states
        .iter()
        .find(|(_, dispatch_state)| dispatch_state.key == key)
        .map(|(_, dispatch_state)| dispatch_state.dispatch_index)
        .unwrap_or(0)
}
