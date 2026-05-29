use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner: ProofObligationOwner,
}

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

impl Default for ProofObligationFact {
    fn default() -> Self {
        Self {
            kind: ProofFactKind::default(),
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
            owner: ProofObligationOwner::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
    pub contract_facts: Arena<ContractProofFact>,
    pub contract_fact_refs: Arena<ContractProofFactRef>,
    pub contract_calls: Arena<ContractCallFact>,
    pub contract_exits: Arena<ContractExitFact>,
}
