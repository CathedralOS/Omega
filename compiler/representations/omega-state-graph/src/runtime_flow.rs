mod builder;

use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::{StateKey, TransitionExpressionRefs};
use omega_core::arena::{Arena, HandleSpan};

pub use builder::build_runtime_flow_plan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFlowPlan {
    pub states: Arena<RuntimeState>,
    pub edges: Arena<RuntimeEdge>,
    pub cycles: Arena<RuntimeCycle>,
    pub cycle_states: Arena<RuntimeState>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeState {
    pub key: StateKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCycle {
    pub states: HandleSpan<RuntimeState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEdge {
    pub from: StateKey,
    pub statement_index: usize,
    pub target: RuntimeTransitionTarget,
    pub continuation: RuntimeTransitionTarget,
    pub guard: TransitionGuard,
    pub expressions: TransitionExpressionRefs,
    pub forms_cycle: bool,
}

impl Default for RuntimeEdge {
    fn default() -> Self {
        Self {
            from: StateKey::default(),
            statement_index: 0,
            target: RuntimeTransitionTarget::None,
            continuation: RuntimeTransitionTarget::None,
            guard: TransitionGuard::Always,
            expressions: TransitionExpressionRefs::default(),
            forms_cycle: false,
        }
    }
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
