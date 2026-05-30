use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWritableRoot {
    pub symbol: SymbolHandle,
    pub kind: StateBorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowAccessKind {
    #[default]
    Read,
    Mutable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowArgumentAccess {
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
    pub kind: StateBorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowCall {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<StateBorrowArgumentAccess>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowLoan {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateBorrowEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for StateBorrowEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowActivation {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWeakening {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
    pub reason: StateBorrowWeakeningReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowSummary {
    pub writable_roots: HandleSpan<StateBorrowWritableRoot>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<StateBorrowCall>,
    pub active_loans: HandleSpan<StateBorrowLoan>,
    pub activations: HandleSpan<StateBorrowActivation>,
    pub weakenings: HandleSpan<StateBorrowWeakening>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphBorrowRoots {
    pub writable_roots: Arena<StateBorrowWritableRoot>,
    pub access_segments: Arena<omega_facts::PlaceSegment>,
    pub argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub calls: Arena<StateBorrowCall>,
    pub loans: Arena<StateBorrowLoan>,
    pub activations: Arena<StateBorrowActivation>,
    pub weakenings: Arena<StateBorrowWeakening>,
}
