mod builder;

use omega_control_flow::{StateKey, TransitionExpressionRefs};
use omega_core::arena::{Arena, HandleSpan};

pub use builder::{build_runtime_flow_plan, build_runtime_flow_plan_with_state_calls};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeFlowPlan {
    pub states: Arena<RuntimeState>,
    pub edges: Arena<RuntimeEdge>,
    pub cycles: Arena<RuntimeCycle>,
    pub cycle_states: Arena<RuntimeState>,
}

impl RuntimeFlowPlan {
    pub fn with_capacity(
        state_capacity: usize,
        edge_capacity: usize,
        cycle_capacity: usize,
        cycle_state_capacity: usize,
    ) -> Self {
        Self {
            states: Arena::with_capacity(state_capacity),
            edges: Arena::with_capacity(edge_capacity),
            cycles: Arena::with_capacity(cycle_capacity),
            cycle_states: Arena::with_capacity(cycle_state_capacity),
        }
    }
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
            expressions: TransitionExpressionRefs::default(),
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeStateCallEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub target_key: StateKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTransitionTarget {
    State { key: StateKey },
    Terminal,
    None,
    Unknown,
}

impl Default for RuntimeTransitionTarget {
    fn default() -> Self {
        Self::None
    }
}
