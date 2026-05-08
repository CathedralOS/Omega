use crate::control_flow::StateKey;
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDispatchPlan {
    pub states: Arena<DispatchState>,
    pub edges: Arena<DispatchEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchState {
    pub key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub dispatch_index: u32,
    pub label: String,
    pub edges: HandleSpan<DispatchEdge>,
}

impl Default for DispatchState {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            dispatch_index: 0,
            label: String::new(),
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEdge {
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub guard: TransitionGuard,
    pub forms_cycle: bool,
}

impl Default for DispatchEdge {
    fn default() -> Self {
        Self {
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            guard: TransitionGuard::Always,
            forms_cycle: false,
        }
    }
}
