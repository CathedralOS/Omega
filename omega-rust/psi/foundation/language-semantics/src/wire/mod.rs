/// A requested compatibility property, not an executable declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WireCompatibilityFact {
    Readable,
    Writable,
    PreserveUnknown,
    Canonical,
    CompleteMigration,
}

impl WireCompatibilityFact {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Readable" => Some(Self::Readable),
            "Writable" => Some(Self::Writable),
            "PreserveUnknown" => Some(Self::PreserveUnknown),
            "Canonical" => Some(Self::Canonical),
            "CompleteMigration" => Some(Self::CompleteMigration),
            _ => None,
        }
    }
}

/// Directional facts requested for one channel/store edge and format lineage.
/// Build supplies these declarations; Psi checking decides whether they hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCompatibilityDemand {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub require_readable: bool,
    pub require_writable: bool,
    pub require_unknown_preservation: bool,
    pub require_canonical: bool,
    pub require_complete_migration: bool,
}

/// An inclusive scalar range established while decoding untrusted wire bytes.
///
/// The bounds are normalized compile-time integers. `signed` selects the
/// comparison algebra used after the wire scalar has been decoded at its
/// declared width. A missing range remains the ZII/no-extra-check case on the
/// wire-read operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireScalarRange {
    pub minimum: i64,
    pub maximum: i64,
    pub signed: bool,
}
