//! Trust and provider-qualification report records, validation, and presentation.

use diagnostics::Diagnostic;

#[cfg(test)]
mod tests;

impl TrustReport {
    /// Validate exact target/report joins independently from filesystem output.
    /// Observation suppression must not suppress these consistency checks.
    pub fn validate(&self) -> Result<(), Diagnostic> {
        for row in &self.provider_requirements {
            row.realization
                .validate_reported_target(&row.target)
                .map_err(Diagnostic::error)?;
        }
        Ok(())
    }
}

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
    pub machine_contract_commitment: Option<checked_trees::MachineContractCommitment>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustCrashRouteGuard {
    Truth,
    PredicateIdentity(Vec<u8>),
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
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact package owning the nominal provider type, when one exists.
    pub provider_type_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Exact compiler-derived package provenance of the realizing machine.
    /// `None` is explicit unbound provenance and is never repaired from names.
    pub provider_origin_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Legacy readable provider-origin label. Diagnostic only.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact package owning the selected service schema.
    pub service_schema_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Compact report coordinate for the evaluated calling contract.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Strong commitment to the exact evaluated calling contract.
    pub calling_plan_commitment: Option<typed_trees::typed_trees::BoundaryCallingPlanCommitment>,
    pub selected: bool,
    pub requirement_owner: String,
    /// Exact package owning the inherited/direct requirement declaration.
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
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
    /// One evaluated, target-normalized foreign locator with its evaluation
    /// receipt. The report renders its raw byte coordinates; no string-backed
    /// import realization exists any more.
    Import {
        evaluated: effects::provider_plan::EvaluatedForeignImport,
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
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    },
}

impl TrustProviderRealization {
    /// Compact compatibility report for the exact foreign locator retained by
    /// this trust row. Other realization cases have no locator report.
    pub fn foreign_locator_compatibility_report_identity(&self) -> Option<u64> {
        match self {
            Self::Import { evaluated } => Some(
                evaluated
                    .locator()
                    .non_authoritative_compatibility_fingerprint(),
            ),
            _ => None,
        }
    }

    fn validate_reported_target(&self, reported_target: &str) -> Result<(), String> {
        let Self::Import { evaluated } = self else {
            return Ok(());
        };
        let locator = evaluated.locator();
        let locator_target = locator.target().target_name();
        if reported_target != locator_target {
            return Err(format!(
                "normalized foreign locator 0x{:016x} targets `{locator_target}`, but its trust row reports target `{reported_target}`",
                locator.non_authoritative_compatibility_fingerprint(),
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn report_text(&self) -> String {
        match self {
            Self::Import { evaluated } => {
                let locator = evaluated.locator();
                let identity = locator.non_authoritative_compatibility_fingerprint();
                let target = locator.target().target_name();
                let receipt = hex_bytes(&evaluated.receipt().identity_digest());
                match locator.locator() {
                    effects::ForeignLocatorCandidate::PeByName { library, export } => {
                        format!(
                            "normalized import PeByName [{identity:016x}] receipt [{receipt}] target `{target}` library bytes {} export bytes {}",
                            hex_bytes(library),
                            hex_bytes(export),
                        )
                    }
                    effects::ForeignLocatorCandidate::PeByOrdinal { library, ordinal } => {
                        format!(
                            "normalized import PeByOrdinal [{identity:016x}] receipt [{receipt}] target `{target}` library bytes {} ordinal {ordinal}",
                            hex_bytes(library),
                        )
                    }
                    effects::ForeignLocatorCandidate::ElfVersioned {
                        object,
                        symbol,
                        version,
                    } => format!(
                        "normalized import ElfVersioned [{identity:016x}] receipt [{receipt}] target `{target}` object bytes {} symbol bytes {} version bytes {}",
                        hex_bytes(object),
                        hex_bytes(symbol),
                        hex_bytes(version),
                    ),
                    effects::ForeignLocatorCandidate::MachODylibSymbol {
                        install_name,
                        symbol,
                    } => format!(
                        "normalized import MachODylibSymbol [{identity:016x}] receipt [{receipt}] target `{target}` install-name bytes {} symbol bytes {}",
                        hex_bytes(install_name),
                        hex_bytes(symbol),
                    ),
                }
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

#[cfg(test)]
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
    pub provider_plan_digest: effects::provider_plan::ProviderPlanDigest,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact package owning the nominal provider type, when one exists.
    pub provider_type_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Exact compiler-derived package provenance of the realizing machine,
    /// independent from grant status. `None` remains explicit unbound
    /// provenance.
    pub provider_origin_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Legacy readable provider-origin label. Diagnostic only.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact package owning the selected service schema.
    pub service_schema_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Compact report coordinate for the evaluated calling contract.
    pub calling_plan_report_fingerprint: Option<u64>,
    /// Strong commitment to the exact evaluated calling contract.
    pub calling_plan_commitment: Option<typed_trees::typed_trees::BoundaryCallingPlanCommitment>,
    pub selected: bool,
    /// Readable semantic owner of the exact requirement. This remains
    /// separate from the canonical overload identity because an inherited
    /// requirement's owner can differ from the selected service schema.
    pub requirement_owner: String,
    /// Exact package owning the inherited/direct requirement declaration.
    pub requirement_owner_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
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
    pub instance_contract_commitment: checked_trees::MachineContractCommitment,
    pub type_argument_identities: Vec<String>,
    pub const_argument_identities: Vec<String>,
    /// Compatibility/report coordinates corresponding positionally to the
    /// strong commitments below.
    pub machine_argument_contract_report_fingerprints: Vec<u64>,
    pub machine_argument_contract_commitments: Vec<checked_trees::MachineContractCommitment>,
    /// Compatibility/report coordinates corresponding positionally to the
    /// strong closed-application commitments below.
    pub conformance_argument_report_fingerprints: Vec<u64>,
    pub conformance_argument_commitments:
        Vec<typed_trees::typed_trees::ClosedConformanceApplicationCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustReport {
    /// Historical compact report coordinate for the selected-provider set.
    pub selected_provider_closure_report_fingerprint: u64,
    /// Collision-resistant commitment to the complete selected-provider set.
    pub selected_provider_closure_digest: effects::SelectedProviderClosureDigest,
    pub rows: Vec<TrustReportRow>,
    pub generic_accepted_instances: Vec<TrustGenericAcceptedInstanceRow>,
    pub provider_requirements: Vec<TrustProviderRequirementRow>,
    pub qualifications: Vec<TrustQualificationRow>,
}

impl Default for TrustReport {
    fn default() -> Self {
        let selected_provider_plans = effects::SelectedProviderPlanFacts::default();
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
