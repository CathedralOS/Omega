use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::TransitionGuard;
use omega_state_calls::StateCallLowering;
use omega_state_guards::StateGuardKind;
use omega_state_storage::{StateMutationKind, StateMutationLowering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub branch_key: StateKey,
    pub target_key: StateKey,
    pub edge_order: usize,
    pub guard: TransitionGuard,
    pub resolved_guard: TransitionGuard,
    pub guard_kind: StateGuardKind,
    pub bindings: HandleSpan<RuntimeStraightLineBranchBinding>,
    pub operations: HandleSpan<RuntimeStraightLineBranchOperation>,
}

impl Default for RuntimeStraightLineBranchExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            statement_index: 0,
            branch_key: StateKey::default(),
            target_key: StateKey::default(),
            edge_order: 0,
            guard: TransitionGuard::Always,
            resolved_guard: TransitionGuard::Always,
            guard_kind: StateGuardKind::Always,
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub expression: ExpressionHandle,
    pub kind: RuntimeStraightLineBranchBindingKind,
}

impl Default for RuntimeStraightLineBranchBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
            expression: ExpressionHandle::invalid(),
            kind: RuntimeStraightLineBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchBindingKind {
    #[default]
    BranchParameter,
    TargetParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchOperation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeStraightLineBranchOperationKind,
}

impl Default for RuntimeStraightLineBranchOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            kind: RuntimeStraightLineBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeStraightLineBranchOperationKind {
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: ExpressionHandle,
        value: ExpressionHandle,
    },
    StateCall {
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalData,
    #[default]
    Other,
}
