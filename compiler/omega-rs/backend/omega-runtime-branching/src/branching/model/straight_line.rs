use omega_control_flow::StateKey;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_guards::StateGuardKind;
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use psi_arena::HandleSpan;
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchExpansion {
    pub scope_id: u32,
    pub child_scope_id: u32,
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub branch_key: StateKey,
    pub target_key: StateKey,
    pub target_statement_index: usize,
    pub edge_order: usize,
    pub guard: ExpressionHandle,
    pub resolved_guard: ExpressionHandle,
    pub guard_kind: StateGuardKind,
    pub local_guard_kind: StateGuardKind,
    pub role: StateCallRole,
    pub call_ordinal: usize,
    pub target_value: ExpressionHandle,
    pub bindings: HandleSpan<RuntimeStraightLineBranchBinding>,
    pub operations: HandleSpan<RuntimeStraightLineBranchOperation>,
}

impl Default for RuntimeStraightLineBranchExpansion {
    fn default() -> Self {
        Self {
            scope_id: 0,
            child_scope_id: 0,
            dispatch_index: 0,
            source_key: StateKey::default(),
            statement_index: 0,
            branch_key: StateKey::default(),
            target_key: StateKey::default(),
            target_statement_index: 0,
            edge_order: 0,
            guard: ExpressionHandle::invalid(),
            resolved_guard: ExpressionHandle::invalid(),
            guard_kind: StateGuardKind::Always,
            local_guard_kind: StateGuardKind::Always,
            role: StateCallRole::Statement,
            call_ordinal: 0,
            target_value: ExpressionHandle::invalid(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStraightLineBranchBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: Identifier,
    pub expression: ExpressionHandle,
    pub kind: RuntimeStraightLineBranchBindingKind,
}

impl Default for RuntimeStraightLineBranchBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: Identifier::default(),
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
    HostCall,
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: ExpressionHandle,
        value: ExpressionHandle,
    },
    StateCall {
        role: StateCallRole,
        call_ordinal: usize,
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalData,
    #[default]
    Other,
}
