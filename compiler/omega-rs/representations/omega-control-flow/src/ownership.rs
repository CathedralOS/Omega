use omega_core::arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;

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
    pub root: psi_facts::PlaceRoot,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateDropEvent {
    pub source: StateOwnershipEventSource,
    pub root: psi_facts::PlaceRoot,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatePermissionEvent {
    pub source: psi_language_semantics::PermissionEventSource,
    pub kind: psi_language_semantics::PermissionEventKind,
    pub multiplicity: psi_language_semantics::Multiplicity,
    pub access: psi_language_semantics::PermissionAccess,
    pub claim_identity: psi_language_semantics::PermissionClaimIdentity,
    pub provenance: psi_language_semantics::PermissionProvenance,
    pub root: psi_facts::PlaceRoot,
    pub segments: HandleSpan<psi_facts::PlaceSegment>,
    pub obligation_live: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateOwnershipSummary {
    pub moves: HandleSpan<StateMoveEvent>,
    pub drops: HandleSpan<StateDropEvent>,
    pub permissions: HandleSpan<StatePermissionEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowOwnershipRoots {
    pub segments: Arena<psi_facts::PlaceSegment>,
    pub moves: Arena<StateMoveEvent>,
    pub drops: Arena<StateDropEvent>,
    pub permissions: Arena<StatePermissionEvent>,
}

impl ControlFlowOwnershipRoots {
    pub fn with_roots(
        segments: Arena<psi_facts::PlaceSegment>,
        moves: Arena<StateMoveEvent>,
        drops: Arena<StateDropEvent>,
        permissions: Arena<StatePermissionEvent>,
    ) -> Self {
        Self {
            segments,
            moves,
            drops,
            permissions,
        }
    }
}
