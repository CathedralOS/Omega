//! Private schema projections and directional verdicts used by wire admission.
//! These are working data, not published reports or trust evidence.

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct WireShape {
    pub name: String,
    pub encoding: Option<String>,
    pub fields: Vec<WireField>,
    pub cases: Vec<WireCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WireField {
    pub number: u64,
    pub relevance: language_core::BindingRelevance,
    pub type_display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WireCase {
    pub number: u64,
    pub payload_fields: Vec<WireField>,
}

pub(super) struct WireDemandVerdict {
    pub edge: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub readability: WireFact,
    pub writability: WireFact,
    pub unknown_preservation: WireFact,
    pub canonicality: WireFact,
    pub migration_coverage: WireFact,
    pub satisfied: bool,
}

pub(super) struct WireFact {
    pub required: bool,
    pub satisfied: bool,
    pub detail: String,
}
