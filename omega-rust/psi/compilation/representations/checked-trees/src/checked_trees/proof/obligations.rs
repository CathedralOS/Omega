use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofFactKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner: ProofObligationOwner,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofObligationOwner {
    #[default]
    Unknown,
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    MachineOwnedData {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    StateReturn {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    TransitionParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
}

/// A completed integer assignment judgment, before the authored store occurs.
/// Retains the exact source occurrence and range; deferred invariant windows
/// and non-integer constraints confer no authority through this row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedIntegerAssignmentDecision {
    /// This judgment establishes an atomic resident, not an ordinary store.
    pub atomic_resident: bool,
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub target: typed_trees::expression::ExpressionHandle,
    pub value: typed_trees::expression::ExpressionHandle,
    pub target_type: typed_trees::types::TypeReferenceHandle,
    pub minimum: numerics::bignum::BigInt,
    pub maximum: numerics::bignum::BigInt,
}
