use omega_control_flow::StateKey;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::statement::TransitionGuard;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::{StateGuardLowering, StateGuardOperandStorage, StateGuardOperator};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchLoopPlan {
    pub needed: bool,
    pub entry_dispatch_index: u32,
    pub terminal_dispatch_index: u32,
    pub current_state_slot: String,
    pub next_state_slot: String,
    pub cases: Arena<RuntimeDispatchLoopCase>,
    pub edges: Arena<RuntimeDispatchLoopEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchLoopCase {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub label: String,
    pub operation_count: usize,
    pub edges: HandleSpan<RuntimeDispatchLoopEdge>,
}

impl Default for RuntimeDispatchLoopCase {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            dispatch_index: 0,
            label: String::new(),
            operation_count: 0,
            edges: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchLoopEdge {
    pub order: usize,
    pub statement_index: usize,
    pub target: RuntimeTransitionTarget,
    pub target_dispatch_index: u32,
    pub target_arguments: HandleSpan<ExpressionHandle>,
    pub continuation: RuntimeTransitionTarget,
    pub continuation_dispatch_index: u32,
    pub continuation_arguments: HandleSpan<ExpressionHandle>,
    pub guard: TransitionGuard,
    pub guard_lowering: StateGuardLowering,
    pub guard_operator: StateGuardOperator,
    pub guard_storage: StateGuardOperandStorage,
    pub guard_byte_offset: usize,
    pub guard_right_storage: StateGuardOperandStorage,
    pub guard_right_byte_offset: usize,
    pub guard_byte_size: usize,
    pub guard_expected_value: i64,
    pub guard_has_storage: bool,
    pub guard_has_right_storage: bool,
    pub action: RuntimeDispatchLoopAction,
    pub forms_cycle: bool,
}

impl Default for RuntimeDispatchLoopEdge {
    fn default() -> Self {
        Self {
            order: 0,
            statement_index: 0,
            target: RuntimeTransitionTarget::None,
            target_dispatch_index: 0,
            target_arguments: HandleSpan::empty(),
            continuation: RuntimeTransitionTarget::None,
            continuation_dispatch_index: 0,
            continuation_arguments: HandleSpan::empty(),
            guard: TransitionGuard::Always,
            guard_lowering: StateGuardLowering::NoOp,
            guard_operator: StateGuardOperator::None,
            guard_storage: StateGuardOperandStorage::Unknown,
            guard_byte_offset: 0,
            guard_right_storage: StateGuardOperandStorage::Unknown,
            guard_right_byte_offset: 0,
            guard_byte_size: 0,
            guard_expected_value: 0,
            guard_has_storage: false,
            guard_has_right_storage: false,
            action: RuntimeDispatchLoopAction::Unknown,
            forms_cycle: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeDispatchLoopAction {
    EnterState,
    Terminate,
    #[default]
    Unknown,
}
