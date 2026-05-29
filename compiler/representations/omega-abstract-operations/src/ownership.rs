use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractOwnershipEventSource {
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

impl Default for AbstractOwnershipEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractMoveEvent {
    pub source_key: StateKey,
    pub source: AbstractOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractDropEvent {
    pub source_key: StateKey,
    pub source: AbstractOwnershipEventSource,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractOwnershipSummary {
    pub segments: Arena<omega_facts::PlaceSegment>,
    pub moves: Arena<AbstractMoveEvent>,
    pub drops: Arena<AbstractDropEvent>,
}

impl AbstractOwnershipSummary {
    pub fn with_capacity(
        segment_capacity: usize,
        move_capacity: usize,
        drop_capacity: usize,
    ) -> Self {
        Self {
            segments: Arena::with_capacity(segment_capacity),
            moves: Arena::with_capacity(move_capacity),
            drops: Arena::with_capacity(drop_capacity),
        }
    }
}
