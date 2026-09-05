use arena::HandleSpan;
use symbols::SymbolHandle;

/// The permission/resource algebra established by the multiplicity checker.
/// This records the semantic role of an event and is the sole ownership-event
/// source for later checked-IR consumers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowPermissionEventFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub source: language_semantics::PermissionEventSource,
    pub kind: language_semantics::PermissionEventKind,
    pub multiplicity: language_semantics::Multiplicity,
    pub access: language_semantics::PermissionAccess,
    pub claim_identity: language_semantics::PermissionClaimIdentity,
    pub provenance: language_semantics::PermissionProvenance,
    pub root: facts::PlaceRoot,
    pub segments: HandleSpan<facts::PlaceSegment>,
    /// Inactive sum alternatives establish/transfer a carrier while carrying
    /// no payload debt. Keep the event and record whether an obligation existed.
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
        segments: HandleSpan<facts::PlaceSegment>,
    },
    Established {
        claim_identity: language_semantics::PermissionClaimIdentity,
        provenance: language_semantics::PermissionProvenance,
    },
}

/// One output-path entry in a checked state's normalized claim outcome map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeEntryFact {
    pub output_segments: HandleSpan<facts::PlaceSegment>,
    pub source: FlowClaimOutcomeSource,
}

/// Path-indexed live-claim mapping published by one checked state result.
/// Statically inactive case alternatives are intentionally absent. Absence of
/// the map means the state has no live linear result frontier or could not
/// prove one unique mapping for every possibly-live output claim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowClaimOutcomeMapFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub entries: HandleSpan<FlowClaimOutcomeEntryFact>,
}
