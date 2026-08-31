use std::path::{Path, PathBuf};

use omega_core::allocations::AllocationDelta;
mod artifact_writer;
mod calling_plan_json;
#[cfg(any(test, feature = "external-root-report"))]
mod external_root_report;
mod timing_report;
mod trust_report;

pub use artifact_writer::ArtifactWriter;
pub use calling_plan_json::value_placement_json;
#[cfg(any(test, feature = "external-root-report"))]
pub use external_root_report::external_root_manifest_json;

fn html_report(title: &str, contents: &str) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>");
    html.push_str(&escape_html(title));
    html.push_str("</title>\n<style>\n");
    html.push_str(REPORT_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<aside>\n<h1>");
    html.push_str(&escape_html(title));
    html.push_str("</h1>\n");
    push_report_nav(&mut html);
    html.push_str("</aside>\n<main><pre>");
    html.push_str(&escape_html(contents));
    html.push_str("</pre></main>\n</body>\n</html>\n");
    html
}

fn push_report_nav(html: &mut String) {
    html.push_str("<nav class=\"phase-nav\" aria-label=\"Pipeline stages\"><a target=\"_top\" href=\"00_pipeline.html\">Index</a>");
    for (number, label, id) in REPORT_LINKS {
        html.push_str("<a target=\"_top\" href=\"00_pipeline.html#");
        html.push_str(&escape_html(id));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span> ");
        html.push_str(&escape_html(label));
        html.push_str("</a>");
    }
    html.push_str("</nav>\n");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const REPORT_LINKS: &[(&str, &str, &str)] = &[
    ("00", "Timings", "timings"),
    ("02", "Syntax", "syntax"),
    ("03", "Symbols", "symbols"),
    ("04", "Typed", "typed"),
    ("05", "Checked", "checked"),
    ("06", "State Graph", "state-graph"),
    ("07", "Control Flow", "control-flow"),
    ("08", "Abstract Operations", "abstract-operations"),
    ("09", "Target Operations", "target-operations"),
    (
        "10",
        "Assigned Target Operations",
        "assigned-target-operations",
    ),
    ("11", "Machine Instructions", "machine-instructions"),
    ("12", "Emission", "emission"),
];

const REPORT_STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
}
* { box-sizing: border-box; }
body {
  min-height: 100vh;
  margin: 0;
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
  color: var(--text);
  display: grid;
  grid-template-columns: minmax(280px, 22vw) 1fr;
  font: 14px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
aside {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  min-height: 100vh;
  padding: 18px;
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
.phase-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.phase-nav a {
  border: 1px solid #303d50;
  border-radius: 999px;
  color: #d8e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 7px 9px;
  text-decoration: none;
}
.phase-nav a:hover { background: #263247; border-color: #8ab4ff; }
.phase-nav span { color: var(--muted); }
main {
  min-width: 0;
  overflow: auto;
  padding: 28px;
}
pre {
  background: rgba(13, 17, 23, 0.82);
  border: 1px solid #283343;
  border-radius: 18px;
  color: #d8e2ef;
  line-height: 1.45;
  margin: 0;
  min-height: calc(100vh - 56px);
  overflow: auto;
  padding: 24px;
  white-space: pre;
}
"#;

fn temp_path_for(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-artifact"),
        std::process::id()
    ))
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

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

/// The chapter-10 TRUST REPORT (GR5): one row per admitted semantic
/// commitment, carrying its provenance tier. Dev-active rows (own-package
/// claims, not yet root-granted) carry the STANDING WARNING the grant
/// locality rule promises; root-granted rows name the exact accepted-machine
/// or selected-provider grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustReportRow {
    /// The commitment, consumer-rendered (`accepted fact: admitted`).
    pub commitment: String,
    /// `own-package (dev-active)` or `root grant`.
    pub provenance: String,
    /// Compact report coordinate for one local accepted-machine contract.
    /// Provider commitments have no machine contract and retain `None` rather
    /// than a synthesized identity.
    pub machine_contract_report_fingerprint: Option<u64>,
    /// Strong commitment to the exact accepted-machine contract.
    pub machine_contract_commitment: Option<psi_checked_trees::MachineContractCommitment>,
    /// Compact report coordinate for one generic accepted-machine template.
    /// Non-generic accepted machines and every other row retain `None`.
    pub machine_template_report_fingerprint: Option<u64>,
    /// Exact published service-reach ceiling for one local accepted machine.
    /// `Some(Vec::new())` is the explicit public negative guarantee; rows that
    /// do not describe a local accepted machine retain `None`.
    pub machine_service_reach: Option<Vec<String>>,
    /// Exact published direct synchronous invocation ceiling for one local
    /// accepted machine. `Some(Vec::new())` is explicit public omission; rows
    /// that do not describe a local accepted machine retain `None`.
    pub machine_synchronous_invocations: Option<Vec<String>>,
    /// Exact published suspension ceiling for one local accepted machine.
    /// `Some(false)` is the public negative guarantee; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_may_suspend: Option<bool>,
    /// Exact published worker-blocking ceiling for one local accepted machine.
    /// `Some(false)` is the public negative guarantee; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_may_block: Option<bool>,
    /// Exact premise-free published termination guarantee for one local
    /// accepted machine. `Some(false)` is published `NoGuarantee`; rows that
    /// do not describe a local accepted machine retain `None`.
    pub machine_terminates_guarantee: Option<bool>,
    /// Exact canonical published crash buckets for one local accepted machine.
    /// `Some(Vec::new())` is the public no-crash ceiling; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_crash_routes: Option<Vec<TrustCrashRouteBucket>>,
    /// Dev-active rows warn until the root grants them.
    pub standing_warning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustCrashCause {
    Trap,
    Abort,
}

impl TrustCrashCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trap => "Trap",
            Self::Abort => "Abort",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustCrashRouteGuard {
    Truth,
    PredicateIdentity(Vec<u8>),
}

impl TrustCrashRouteGuard {
    fn report_text(&self) -> String {
        match self {
            Self::Truth => "true".to_owned(),
            Self::PredicateIdentity(bytes) => {
                let mut identity = String::from("0x");
                for byte in bytes {
                    identity.push_str(&format!("{byte:02x}"));
                }
                identity
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustCrashRouteBucket {
    pub cause: TrustCrashCause,
    pub alternative_guards: Vec<TrustCrashRouteGuard>,
}

/// One exact requirement supplied by a normalized provider-plan row.
///
/// This is the claim-free provider blast-radius carrier: readable names remain
/// separate from canonical overload identity, and admission provenance is
/// copied rather than inferred from the requirement or schema spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProviderRequirementRow {
    pub provider_plan: String,
    /// Historical compact report coordinate for the provider plan.
    pub provider_plan_report_fingerprint: u64,
    /// Collision-resistant commitment to the complete normalized plan.
    pub provider_plan_digest: omega_effects::provider_plan::ProviderPlanDigest,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact package owning the nominal provider type, when one exists.
    pub provider_type_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Exact compiler-derived package provenance of the realizing machine.
    /// `None` is explicit unbound provenance and is never repaired from names.
    pub provider_origin_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Legacy readable provider-origin label. Diagnostic only.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact package owning the selected service schema.
    pub service_schema_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Compact report coordinate for the evaluated calling contract.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Strong commitment to the exact evaluated calling contract.
    pub calling_plan_commitment:
        Option<psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment>,
    pub selected: bool,
    pub requirement_owner: String,
    /// Exact package owning the inherited/direct requirement declaration.
    pub requirement_owner_package_identity: Option<psi_core::PackageKeyIdentity>,
    pub requirement_identity: String,
    pub method: String,
    /// Exact positional normalized semantic parameter identities.
    pub parameter_type_identities: Vec<String>,
    /// Exact normalized semantic result identity; `None` means no result.
    pub result_type_identity: Option<String>,
    /// Exact normalized boundary-service reach for this requirement.
    pub service_reach: Vec<String>,
    /// Exact normalized direct invocation bindings. This is not reach closure.
    pub synchronous_invocations: Vec<String>,
    pub may_suspend: bool,
    pub may_block: bool,
    /// Exact existing public `terminates;` guarantee on the bodyless
    /// requirement. Private ranking witnesses remain excluded.
    pub terminates_guarantee: bool,
    /// Exact public progress-premise schemas retained by the requirement.
    /// Provider-receiver subjects remain visibly build-bound rather than being
    /// rendered as caller parameters or silently dropped.
    pub termination_premises: Vec<TrustProgressPremiseRow>,
    pub realization: TrustProviderRealization,
    pub provenance: String,
    pub grant_selectors: Vec<String>,
    pub standing_warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProgressPremiseRow {
    pub profile: String,
    pub subject: TrustProgressPremiseSubject,
    pub subject_projections: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustProgressPremiseSubject {
    ProviderReceiver,
    Parameter(usize),
}

/// Exact normalized realization selected by one provider-plan row.
///
/// This remains structured so the durable trust artifact distinguishes checked
/// Omega adapters from opaque/raw leaves without parsing a debug rendering or
/// inferring mechanism from the provider name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustProviderRealization {
    Import {
        locator: omega_effects::NormalizedForeignLocator,
    },
    /// Temporary source `via Binding::DllImport("library", "symbol")`
    /// realization. This remains visibly distinct from an evaluated,
    /// target-normalized foreign locator.
    StringBackedImportBootstrap {
        library: String,
        symbol: String,
    },
    Syscall {
        number: i64,
    },
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    VtableField {
        table: String,
        field: String,
    },
    TableFunction {
        table: String,
        field: String,
    },
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<psi_core::PackageKeyIdentity>,
    },
}

impl TrustProviderRealization {
    /// Compact compatibility report for the exact foreign locator retained by
    /// this trust row. Other realization cases have no locator report.
    pub fn foreign_locator_compatibility_report_identity(&self) -> Option<u64> {
        match self {
            Self::Import { locator } => Some(locator.non_authoritative_compatibility_fingerprint()),
            _ => None,
        }
    }

    fn validate_reported_target(&self, reported_target: &str) -> Result<(), String> {
        let Self::Import { locator } = self else {
            return Ok(());
        };
        let locator_target = locator.target().target_name();
        if reported_target != locator_target {
            return Err(format!(
                "normalized foreign locator 0x{:016x} targets `{locator_target}`, but its trust row reports target `{reported_target}`",
                locator.non_authoritative_compatibility_fingerprint(),
            ));
        }
        Ok(())
    }

    fn report_text(&self) -> String {
        match self {
            Self::Import { locator } => {
                let identity = locator.non_authoritative_compatibility_fingerprint();
                let target = locator.target().target_name();
                match locator.locator() {
                    omega_effects::ForeignLocatorCandidate::PeByName { library, export } => {
                        format!(
                            "normalized import PeByName [{identity:016x}] target `{target}` library bytes {} export bytes {}",
                            hex_bytes(library),
                            hex_bytes(export),
                        )
                    }
                    omega_effects::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        format!(
                            "normalized import PeByOrdinal [{identity:016x}] target `{target}` library bytes {} ordinal {ordinal}",
                            hex_bytes(library),
                        )
                    }
                    omega_effects::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => format!(
                        "normalized import ElfVersioned [{identity:016x}] target `{target}` object bytes {} symbol bytes {} version bytes {}",
                        hex_bytes(object),
                        hex_bytes(symbol),
                        hex_bytes(version),
                    ),
                    omega_effects::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => format!(
                        "normalized import MachODylibSymbol [{identity:016x}] target `{target}` install-name bytes {} symbol bytes {}",
                        hex_bytes(install_name),
                        hex_bytes(symbol),
                    ),
                }
            }
            Self::StringBackedImportBootstrap { library, symbol } => {
                format!("string-backed import bootstrap `{library}` symbol `{symbol}`")
            }
            Self::Syscall { number } => format!("syscall {number}"),
            Self::CompilerIntrinsic { machine } => {
                format!("compiler intrinsic realization `{machine}`")
            }
            Self::VtableSlot { index } => format!("vtable slot {index}"),
            Self::VtableField { table, field } => format!("vtable field `{table}.{field}`"),
            Self::TableFunction { table, field } => {
                format!("table function `{table}.{field}`")
            }
            Self::CheckedAdapter {
                machine_identity, ..
            } => format!("checked adapter `{machine_identity}`"),
        }
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(2 + bytes.len() * 2);
    text.push_str("0x");
    for byte in bytes {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// One exact routed qualification carried by a normalized provider plan.
///
/// These rows keep the durable trust artifact at least as specific as the
/// admitted claim. They are derived from structured provider-plan schema, not
/// parsed from display types or reconstructed from a compact plan identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustQualificationRow {
    pub provider_plan: String,
    /// Historical compact report coordinate for the provider plan.
    pub provider_plan_report_fingerprint: u64,
    /// Collision-resistant commitment to the complete normalized plan.
    pub provider_plan_digest: omega_effects::provider_plan::ProviderPlanDigest,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact package owning the nominal provider type, when one exists.
    pub provider_type_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Exact compiler-derived package provenance of the realizing machine,
    /// independent from grant status. `None` remains explicit unbound
    /// provenance.
    pub provider_origin_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Legacy readable provider-origin label. Diagnostic only.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact package owning the selected service schema.
    pub service_schema_package_identity: Option<psi_core::PackageKeyIdentity>,
    /// Compact report coordinate for the evaluated calling contract.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Strong commitment to the exact evaluated calling contract.
    pub calling_plan_commitment:
        Option<psi_typed_trees::typed_trees::BoundaryCallingPlanCommitment>,
    pub selected: bool,
    /// Readable semantic owner of the exact requirement. This remains
    /// separate from the canonical overload identity because an inherited
    /// requirement's owner can differ from the selected service schema.
    pub requirement_owner: String,
    /// Exact package owning the inherited/direct requirement declaration.
    pub requirement_owner_package_identity: Option<psi_core::PackageKeyIdentity>,
    pub requirement_identity: String,
    pub method: String,
    /// `parameter:N` for an accepted entry claim or `result` for a returned
    /// qualification.
    pub subject: String,
    /// `accepts` or `returns`.
    pub authority_flow: String,
    pub domain: String,
    pub effective_carry: String,
    pub predicate_discharge_required: bool,
    pub provenance: String,
    /// Exact authored root-grant selectors that activated this selected plan.
    /// The provider-plan digest above remains the selected semantic identity;
    /// these strings retain its source-level grant provenance.
    pub grant_selectors: Vec<String>,
    pub standing_warning: bool,
}

/// One checked instantiation of a universal generic accepted machine. These
/// rows consume no additional grant: they retain the exact template identity
/// and selected machine-contract identities for audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustGenericAcceptedInstanceRow {
    pub template_commitment: String,
    /// Historical compact report coordinate for the authored template.
    pub template_report_fingerprint: u64,
    /// Historical compact report coordinate for the specialization tuple.
    /// Exact type/const identities and strong selected-argument commitments
    /// below remain the replay evidence.
    pub instance_report_fingerprint: u64,
    /// Historical compact report coordinate for the checked instance
    /// contract. Authority-bearing consumers use the adjacent commitment.
    pub instance_contract_report_fingerprint: u64,
    /// Domain-separated commitment to the exact checked instance contract.
    pub instance_contract_commitment: psi_checked_trees::MachineContractCommitment,
    pub type_argument_identities: Vec<String>,
    pub const_argument_identities: Vec<String>,
    /// Compatibility/report coordinates corresponding positionally to the
    /// strong commitments below.
    pub machine_argument_contract_report_fingerprints: Vec<u64>,
    pub machine_argument_contract_commitments: Vec<psi_checked_trees::MachineContractCommitment>,
    /// Compatibility/report coordinates corresponding positionally to the
    /// strong closed-application commitments below.
    pub conformance_argument_report_fingerprints: Vec<u64>,
    pub conformance_argument_commitments:
        Vec<psi_typed_trees::typed_trees::ClosedConformanceApplicationCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustReport {
    /// Historical compact report coordinate for the selected-provider set.
    pub selected_provider_closure_report_fingerprint: u64,
    /// Collision-resistant commitment to the complete selected-provider set.
    pub selected_provider_closure_digest: omega_effects::SelectedProviderClosureDigest,
    pub rows: Vec<TrustReportRow>,
    pub generic_accepted_instances: Vec<TrustGenericAcceptedInstanceRow>,
    pub provider_requirements: Vec<TrustProviderRequirementRow>,
    pub qualifications: Vec<TrustQualificationRow>,
}

impl Default for TrustReport {
    fn default() -> Self {
        let selected_provider_plans = omega_effects::SelectedProviderPlanFacts::default();
        Self {
            selected_provider_closure_report_fingerprint: selected_provider_plans
                .compatibility_report_identity(),
            selected_provider_closure_digest: selected_provider_plans.identity_digest(),
            rows: Vec::new(),
            generic_accepted_instances: Vec::new(),
            provider_requirements: Vec::new(),
            qualifications: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
