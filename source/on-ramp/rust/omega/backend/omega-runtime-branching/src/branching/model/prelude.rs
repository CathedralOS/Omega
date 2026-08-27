use omega_control_flow::StateKey;
use omega_state_calls::{StateCallLowering, StateCallRole};
use omega_state_storage::{StateMutationKind, StateMutationLowering};
use psi_arena::HandleSpan;
use psi_checked_trees::expression::ExpressionHandle;
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchPreludeExpansion {
    pub dispatch_index: u32,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub branch_key: StateKey,
    pub target_key: StateKey,
    /// The spawning state call's role. Guard-role preludes are the callee's
    /// ONLY executor (their splice is skipped); every other role's prelude
    /// coexists with the splice, and the selection layer uses this to skip
    /// re-emitting local-initializer VALUE writes the splice already covers
    /// (the cross-callee let-name collision's internal-op flavor: the
    /// prelude duplicate ran wrong-timed with cross-callee operands).
    pub role: StateCallRole,
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
            role: StateCallRole::default(),
            bindings: HandleSpan::empty(),
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBranchPreludeBinding {
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: Identifier,
    pub expression: ExpressionHandle,
}

impl Default for RuntimeBranchPreludeBinding {
    fn default() -> Self {
        Self {
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: Identifier::default(),
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
