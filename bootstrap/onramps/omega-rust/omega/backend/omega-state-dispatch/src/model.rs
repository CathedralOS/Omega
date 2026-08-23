use omega_control_flow::{StateKey, TransitionExpressionRefs};
use omega_state_graph::{CallResultReturn, RuntimeTransitionTarget};
use psi_arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchPlan {
    pub states: Arena<DispatchState>,
    pub edges: Arena<DispatchEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchState {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub edges: HandleSpan<DispatchEdge>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            dispatch_index: 0,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEdge {
    pub statement_index: usize,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub expressions: TransitionExpressionRefs,
    pub forms_cycle: bool,
    pub call_result: Option<CallResultReturn>,
}

impl Default for DispatchEdge {
    fn default() -> Self {
        Self {
            statement_index: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            expressions: TransitionExpressionRefs::default(),
            forms_cycle: false,
            call_result: None,
        }
    }
}
