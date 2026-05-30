use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateOwnershipEventSource {
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

impl Default for StateOwnershipEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateMoveEvent {
    pub source: StateOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDropEvent {
    pub source: StateOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateOwnershipSummary {
    pub moves: HandleSpan<StateMoveEvent>,
    pub drops: HandleSpan<StateDropEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowOwnershipRoots {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub moves: Arena<StateMoveEvent>,
    pub drops: Arena<StateDropEvent>,
}
