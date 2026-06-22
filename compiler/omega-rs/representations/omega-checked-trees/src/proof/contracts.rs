use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

use crate::CheckedValueOrigin;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactKind {
    #[default]
    Requires,
    Ensures,
    Boundary,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ContractProofFactOwner {
    #[default]
    Unknown,
    Machine {
        machine_symbol: SymbolHandle,
    },
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    StateSignature {
        owner_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    OperatorUse {
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        operator_symbol: SymbolHandle,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFact {
    pub kind: ContractProofFactKind,
    pub owner: ContractProofFactOwner,
    pub fact: Handle<omega_typed_trees::domain::ProofFact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContractProofFactRef {
    pub fact: Handle<ContractProofFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractCallFact {
    pub caller_machine_symbol: SymbolHandle,
    pub caller_state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractExitFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub statement_index: usize,
    pub ensures: HandleSpan<ContractProofFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractOperatorUseFact {
    pub expression: ExpressionHandle,
    pub origin: CheckedValueOrigin,
    pub operator_symbol: SymbolHandle,
    pub requires: HandleSpan<ContractProofFactRef>,
    pub ensures: HandleSpan<ContractProofFactRef>,
    pub boundary: HandleSpan<ContractProofFactRef>,
}
