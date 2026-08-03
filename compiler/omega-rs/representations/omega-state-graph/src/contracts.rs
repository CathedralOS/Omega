use psi_arena::{Arena, Handle, HandleSpan};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateContractFactKind {
    #[default]
    Requires,
    Ensures,
    Boundary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractFactRef {
    pub kind: StateContractFactKind,
    pub fact: Handle<psi_typed_trees::domain::ProofFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractCall {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<StateContractFactRef>,
    pub ensures: HandleSpan<StateContractFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractExit {
    pub statement_index: usize,
    pub ensures: HandleSpan<StateContractFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractSummary {
    pub calls: HandleSpan<StateContractCall>,
    pub exits: HandleSpan<StateContractExit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphContractRoots {
    pub fact_refs: Arena<StateContractFactRef>,
    pub calls: Arena<StateContractCall>,
    pub exits: Arena<StateContractExit>,
}

impl StateGraphContractRoots {
    pub fn with_roots(
        fact_refs: Arena<StateContractFactRef>,
        calls: Arena<StateContractCall>,
        exits: Arena<StateContractExit>,
    ) -> Self {
        Self {
            fact_refs,
            calls,
            exits,
        }
    }
}
