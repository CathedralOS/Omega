use crate::runtime_dispatch::guards::StateGuardKind;
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

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
    pub leaf_key: StateKey,
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
            leaf_key: StateKey::default(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLeafBranchBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub expression: Expression,
    pub kind: RuntimeLeafBranchBindingKind,
}

impl Default for RuntimeLeafBranchBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
            expression: Expression::Integer(0),
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
    HostCall {
        platform_call: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
        target: Expression,
        value: Expression,
    },
    #[default]
    Other,
}
