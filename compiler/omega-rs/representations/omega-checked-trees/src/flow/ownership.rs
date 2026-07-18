use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowOwnershipEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
    StateExit,
}

impl Default for FlowOwnershipEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowMoveEventFact {
    pub source: FlowOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowDropEventFact {
    pub source: FlowOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

/// The permission/resource algebra established by the multiplicity checker.
/// Unlike the legacy move/drop summary, this records the semantic role of an
/// event and is suitable as the source for later checked-IR consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowPermissionEventKind {
    Establish,
    Transfer,
    Consume,
    AffineDrop,
}

impl Default for FlowPermissionEventKind {
    fn default() -> Self {
        Self::Transfer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowPermissionEventSource {
    StateEntry,
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
    StateExit,
}

impl Default for FlowPermissionEventSource {
    fn default() -> Self {
        Self::StateEntry
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowPermissionEventFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source: FlowPermissionEventSource,
    pub kind: FlowPermissionEventKind,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
    /// `Empty` conditional sums establish/transfer a value while carrying no
    /// payload debt. Keep the event and record whether an obligation existed.
    pub obligation_live: bool,
}
