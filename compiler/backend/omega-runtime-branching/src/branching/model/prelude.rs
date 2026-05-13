use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::name::ProgramName;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_storage::{StateMutationKind, StateMutationLowering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchPreludeExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub branch_key: StateKey,
    pub target_key: StateKey,
    pub bindings: HandleSpan<RuntimeBranchPreludeBinding>,
    pub operations: HandleSpan<RuntimeBranchPreludeOperation>,
}

impl Default for RuntimeBranchPreludeExpansion {
    fn default() -> Self {
        Self {
            dispatch_index: 0,
            source_key: StateKey::default(),
            statement_index: 0,
            branch_key: StateKey::default(),
            target_key: StateKey::default(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchPreludeBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub expression: ExpressionHandle,
}

impl Default for RuntimeBranchPreludeBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
            expression: ExpressionHandle::invalid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchPreludeOperation {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub kind: RuntimeBranchPreludeOperationKind,
}

impl Default for RuntimeBranchPreludeOperation {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            kind: RuntimeBranchPreludeOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeBranchPreludeOperationKind {
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
        role: StateCallRole,
        target_key: StateKey,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalData,
    #[default]
    Other,
}
