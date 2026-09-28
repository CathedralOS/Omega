use crate::name::DiagnosticName;
use crate::types::TypeReference;
use arena::HandleSpan;
use symbols::SymbolHandle;

/// A `wire data` protocol schema: explicit field numbers, retired (reserved)
/// numbers, and historical version eras. Wire schemas describe external
/// representation contracts, not runtime layout, so they are carried as their
/// own root family instead of folding into `DataDefinition`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireSchema {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Source visibility retained independently from schema identity.
    pub is_public: bool,
    pub encoding: Option<DiagnosticName>,
    pub members: HandleSpan<WireMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireMember {
    Field(WireField),
    Reserved(WireReserved),
    Version(WireVersion),
}

impl Default for WireMember {
    fn default() -> Self {
        Self::Reserved(WireReserved::default())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireField {
    pub number: u64,
    pub name: DiagnosticName,
    pub relevance: language_core::BindingRelevance,
    pub type_reference: TypeReference,
}

impl Default for WireField {
    fn default() -> Self {
        Self {
            number: 0,
            name: DiagnosticName::default(),
            relevance: Default::default(),
            type_reference: TypeReference::Unit,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireReserved {
    pub number: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireVersion {
    pub name: DiagnosticName,
    pub members: HandleSpan<WireMember>,
}
