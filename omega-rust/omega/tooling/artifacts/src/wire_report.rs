//! Wire-schema and compatibility observation records assembled by the compiler.

/// Compatibility report for `wire data` protocol schemas (chapter 20): field
/// tables, retired numbers, declared version eras, and per-era verdicts along
/// the VERSION CHAIN (each era against its successor; the newest era against
/// the current schema body). Built from typed trees by the compiler pipeline;
/// this crate only owns the artifact shape and rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProtocolReport {
    pub schemas: Vec<WireSchemaReportEntry>,
    pub demands: Vec<WireCompatibilityDemandReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireSchemaReportEntry {
    pub name: String,
    /// Compact report coordinate for the normalized schema. Compatibility
    /// decisions must compare the retained exact field/case rows instead.
    pub normalized_schema_report_identity: u64,
    /// Whether the compiler exposed generated codec entries for this schema.
    /// Ordinary data may carry both this realization fact and its normalized
    /// reflected schema report coordinate in the same merged row.
    pub synthesized_codec: bool,
    pub encoding: Option<String>,
    pub codec_requirement: Option<String>,
    pub codec_requirement_report_identity: Option<u64>,
    pub encode_requirement: Option<String>,
    pub encode_requirement_report_identity: Option<u64>,
    pub normalized_plan_report_identity: Option<u64>,
    pub encode_obligations: Vec<String>,
    pub realization_origin: Option<WireRealizationOrigin>,
    pub trust_class: Option<WireTrustClass>,
    pub realization_evidence: Vec<String>,
    /// The era discriminator the CURRENT body encodes (decision 10): the
    /// number of declared version blocks (0 for an unversioned schema).
    pub current_era: u64,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<u64>,
    pub cases: Vec<WireCaseReportEntry>,
    pub retired_cases: Vec<u64>,
    pub versions: Vec<WireVersionReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireRealizationOrigin {
    Authored,
    Generated { generator: String },
    Foreign { provider: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireTrustClass {
    Derived,
    Admitted { authority: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireFieldReportEntry {
    pub number: u64,
    pub name: String,
    pub relevance: WireFieldRelevance,
    pub type_display: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireFieldRelevance {
    #[default]
    Relevant,
    Erased,
}

impl WireFieldRelevance {
    pub fn is_erased(self) -> bool {
        matches!(self, Self::Erased)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCaseReportEntry {
    pub number: u64,
    pub name: String,
    pub payload_fields: Vec<WireFieldReportEntry>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireVersionReportEntry {
    pub name: String,
    /// The era discriminator payloads of this declared version carry: its
    /// zero-based position in the declaration-ordered version chain.
    pub era: u64,
    /// The next era in the version chain this era's verdicts compare against:
    /// the following declared version, or `current` for the newest era.
    pub successor: String,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<u64>,
    pub verdicts: WireCompatibilityVerdicts,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityVerdicts {
    pub compatible: Vec<String>,
    /// Cross-era type changes on a stable field number: legal evolution (the
    /// era discriminator selects the old decode table), surfaced as a report
    /// verdict instead of a compile error.
    pub requires_migration: Vec<String>,
    pub reserved: Vec<String>,
    pub incompatible: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityDemandReportEntry {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub codec: String,
    pub unknown_member_behavior: String,
    pub readability: WireCompatibilityFactReport,
    pub writability: WireCompatibilityFactReport,
    pub unknown_preservation: WireCompatibilityFactReport,
    pub canonicality: WireCompatibilityFactReport,
    pub migration_coverage: WireCompatibilityFactReport,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityFactReport {
    pub required: bool,
    pub satisfied: bool,
    pub detail: String,
}
