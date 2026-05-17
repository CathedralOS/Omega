use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_state_calls::StateCallRole;
use omega_state_guards::StateGuardKind;
use omega_state_storage::{StateMutationKind, StateMutationLowering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub branch_key: StateKey,
    pub edge_order: usize,
    pub guard: TransitionGuard,
    pub resolved_guard: TransitionGuard,
    pub guard_kind: StateGuardKind,
    pub role: StateCallRole,
    pub leaf_key: StateKey,
    pub target_value: ExpressionHandle,
    pub bindings: HandleSpan<RuntimeLeafBranchBinding>,
    pub operations: HandleSpan<RuntimeLeafBranchOperation>,
}

impl Default for RuntimeLeafBranchExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            statement_index: 0,
            branch_key: StateKey::default(),
            edge_order: 0,
            guard: TransitionGuard::Always,
            resolved_guard: TransitionGuard::Always,
            guard_kind: StateGuardKind::Always,
            role: StateCallRole::Statement,
            leaf_key: StateKey::default(),
            target_value: ExpressionHandle::invalid(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub expression: ExpressionHandle,
    pub kind: RuntimeLeafBranchBindingKind,
}

impl Default for RuntimeLeafBranchBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
            expression: ExpressionHandle::invalid(),
            kind: RuntimeLeafBranchBindingKind::BranchParameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchBindingKind {
    #[default]
    BranchParameter,
    LeafParameter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchOperation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeLeafBranchOperationKind,
}

impl Default for RuntimeLeafBranchOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            kind: RuntimeLeafBranchOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeLeafBranchOperationKind {
    HostCall,
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: ExpressionHandle,
        value: ExpressionHandle,
    },
    #[default]
    Other,
}
