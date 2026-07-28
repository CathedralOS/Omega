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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowPermissionEventFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source: omega_core::semantics::PermissionEventSource,
    pub kind: omega_core::semantics::PermissionEventKind,
    pub multiplicity: omega_core::semantics::Multiplicity,
    pub access: omega_core::semantics::PermissionAccess,
    pub claim_identity: omega_core::semantics::PermissionClaimIdentity,
    pub provenance: omega_core::semantics::PermissionProvenance,
    pub root: omega_facts::PlaceRoot,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
    /// `Empty` conditional sums establish/transfer a value while carrying no
    /// payload debt. Keep the event and record whether an obligation existed.
    pub obligation_live: bool,
}

/// One normalized source for a claim transferred through a checked state's
/// result. Inputs are relative to the callee's declared parameter frontier;
/// claims established by the checked body retain their exact semantic
/// identity and root-lineage provenance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowClaimOutcomeSource {
    #[default]
    Unknown,
    Input {
        parameter_symbol: SymbolHandle,
        segments: HandleSpan<omega_facts::PlaceSegment>,
    },
    Established {
        claim_identity: omega_core::semantics::PermissionClaimIdentity,
        provenance: omega_core::semantics::PermissionProvenance,
    },
}

/// One output-path entry in a checked state's normalized claim outcome map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeEntryFact {
    pub output_segments: HandleSpan<omega_facts::PlaceSegment>,
    pub source: FlowClaimOutcomeSource,
}

/// Complete path-indexed claim mapping published by one checked state result.
/// Absence means the state has no live linear result frontier or could not
/// prove one unique mapping for every output claim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeMapFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub entries: HandleSpan<FlowClaimOutcomeEntryFact>,
}
