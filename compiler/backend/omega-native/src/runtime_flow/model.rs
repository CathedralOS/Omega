use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFlowPlan {
    pub states: Arena<RuntimeState>,
    pub edges: Arena<RuntimeEdge>,
    pub cycle_states: Arena<RuntimeState>,
    pub cycles: Arena<RuntimeCycle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeState {
    pub key: StateKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEdge {
    pub from: StateKey,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub forms_cycle: bool,
}

impl Default for RuntimeEdge {
    fn default() -> Self {
        Self {
            from: StateKey::default(),
            target: RuntimeTransitionTarget::Terminal,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCycle {
    pub states: HandleSpan<RuntimeState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTransitionTarget {
    State { key: StateKey },
    Terminal,
    None,
    Unknown { name: String },
}

impl Default for RuntimeTransitionTarget {
    fn default() -> Self {
        Self::None
    }
}
