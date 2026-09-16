//! Trust and provider-qualification report records, validation, and presentation.

use diagnostics::Diagnostic;

use crate::ArtifactWriter;

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

impl ArtifactWriter {
    /// GR5: the chapter-10 trust report -- the proof-tier surface the
    /// boundary report does not carry. Written even when empty (an empty
    /// report is the honest "no semantic commitments admitted" statement).
    pub fn write_trust_report(&self, trust_report: &TrustReport) -> Result<(), Diagnostic> {
        trust_report.validate()?;
        let mut output = String::new();
        output.push_str("# Omega Trust\n\n");
        output.push_str(&format!(
            "selected provider closure report fingerprint: {:016x}\n\nselected provider closure digest: {}\n\n",
            trust_report.selected_provider_closure_report_fingerprint,
            hex_bytes(trust_report.selected_provider_closure_digest.as_bytes()),
        ));
        output.push_str(&format!(
            "admitted commitments: {}\n\n",
            trust_report.rows.len()
        ));
        for row in &trust_report.rows {
            output.push_str(&format!("- {} -- {}", row.commitment, row.provenance));
            if let Some(fingerprint) = row.machine_contract_report_fingerprint {
                output.push_str(&format!(
                    " -- machine contract report fingerprint: {fingerprint:016x}"
                ));
            }
            if let Some(commitment) = row.machine_contract_commitment {
                output.push_str(&format!(
                    " -- machine contract commitment: {}",
                    hex_bytes(&commitment.as_bytes())
                ));
            }
            if let Some(fingerprint) = row.machine_template_report_fingerprint {
                output.push_str(&format!(
                    " -- accepted template report fingerprint: {fingerprint:016x}"
                ));
            }
            if let Some(service_reach) = &row.machine_service_reach {
                output.push_str(" -- service reach: ");
                if service_reach.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&service_reach.join(", "));
                }
            }
            if let Some(invocations) = &row.machine_synchronous_invocations {
                output.push_str(" -- synchronous invocations: ");
                if invocations.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&invocations.join(", "));
                }
            }
            if let Some(may_suspend) = row.machine_may_suspend {
                output.push_str(&format!(
                    " -- may suspend: {}",
                    if may_suspend { "yes" } else { "no" }
                ));
            }
            if let Some(may_block) = row.machine_may_block {
                output.push_str(&format!(
                    " -- may block: {}",
                    if may_block { "yes" } else { "no" }
                ));
            }
            if let Some(terminates) = row.machine_terminates_guarantee {
                output.push_str(&format!(
                    " -- termination guarantee: {}",
                    if terminates { "yes" } else { "no" }
                ));
            }
            if let Some(routes) = &row.machine_crash_routes {
                output.push_str(" -- crash routes: ");
                if routes.is_empty() {
                    output.push_str("none");
                } else {
                    for (route_index, route) in routes.iter().enumerate() {
                        if route_index > 0 {
                            output.push_str(", ");
                        }
                        output.push_str(route.cause.as_str());
                        output.push('[');
                        for (guard_index, guard) in route.alternative_guards.iter().enumerate() {
                            if guard_index > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&guard.report_text());
                        }
                        output.push(']');
                    }
                }
            }
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants it (`b.accept_boundary<..>();`)]");
            }
            output.push('\n');
        }
        output.push_str("\n## Generic accepted instances\n\n");
        output.push_str(&format!(
            "generic accepted instances: {}\n\n",
            trust_report.generic_accepted_instances.len()
        ));
        for row in &trust_report.generic_accepted_instances {
            output.push_str(&format!(
                "- accepted template: {} -- template report fingerprint: {:016x} -- instance report fingerprint: {:016x} -- instance contract report fingerprint: {:016x} -- instance contract commitment: {} -- type argument identities: {} -- const argument identities: {} -- machine argument contract report fingerprints: {} -- machine argument contract commitments: {} -- conformance argument report fingerprints: {} -- conformance argument commitments: {}\n",
                row.template_commitment,
                row.template_report_fingerprint,
                row.instance_report_fingerprint,
                row.instance_contract_report_fingerprint,
                hex_bytes(&row.instance_contract_commitment.as_bytes()),
                if row.type_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.type_argument_identities.join(", ")
                },
                if row.const_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.const_argument_identities.join(", ")
                },
                if row.machine_argument_contract_report_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.machine_argument_contract_report_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.machine_argument_contract_commitments.is_empty() {
                    "none".to_owned()
                } else {
                    row.machine_argument_contract_commitments
                        .iter()
                        .map(|commitment| hex_bytes(&commitment.as_bytes()))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.conformance_argument_report_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.conformance_argument_report_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.conformance_argument_commitments.is_empty() {
                    "none".to_owned()
                } else {
                    row.conformance_argument_commitments
                        .iter()
                        .map(|commitment| hex_bytes(&commitment.as_bytes()))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        output.push_str("\n## Provider requirements\n\n");
        output.push_str(&format!(
            "provider requirements: {}\n\n",
            trust_report.provider_requirements.len()
        ));
        for row in &trust_report.provider_requirements {
            output.push_str(&format!(
                "- provider plan: {} -- plan report fingerprint: {:016x} -- plan digest: {} -- provider type: {} -- provider type package: {} -- target: {} -- provider origin package: {} -- provider package key: {} -- service schema: {} -- service schema package: {} -- calling plan report fingerprint: {} -- calling plan commitment: {} -- selected: {} -- requirement owner: {} -- requirement owner package: {} -- requirement identity: {} -- method: {} -- parameter types: {} -- result type: {} -- service reach: {} -- synchronous invocations: {} -- may suspend: {} -- may block: {} -- termination guarantee: {} -- progress premises: {} -- realization: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_report_fingerprint,
                hex_bytes(row.provider_plan_digest.as_bytes()),
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                package_key_text(row.provider_type_package_identity),
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                package_key_text(row.provider_origin_package_identity),
                row.service_schema,
                package_key_text(row.service_schema_package_identity),
                row.calling_plan_report_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                row.calling_plan_commitment.map_or_else(
                    || "<none>".to_owned(),
                    |commitment| hex_bytes(&commitment.as_bytes()),
                ),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                package_key_text(row.requirement_owner_package_identity),
                row.requirement_identity,
                row.method,
                if row.parameter_type_identities.is_empty() {
                    "<none>".to_owned()
                } else {
                    row.parameter_type_identities.join(", ")
                },
                row.result_type_identity.as_deref().unwrap_or("<none>"),
                if row.service_reach.is_empty() {
                    "none".to_owned()
                } else {
                    row.service_reach.join(", ")
                },
                if row.synchronous_invocations.is_empty() {
                    "none".to_owned()
                } else {
                    row.synchronous_invocations.join(", ")
                },
                if row.may_suspend { "yes" } else { "no" },
                if row.may_block { "yes" } else { "no" },
                if row.terminates_guarantee { "yes" } else { "no" },
                progress_premises_text(&row.termination_premises),
                row.realization.report_text(),
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        output.push_str("\n## Routed qualifications\n\n");
        output.push_str(&format!(
            "routed qualifications: {}\n\n",
            trust_report.qualifications.len()
        ));
        for row in &trust_report.qualifications {
            output.push_str(&format!(
                "- provider plan: {} -- plan report fingerprint: {:016x} -- plan digest: {} -- provider type: {} -- provider type package: {} -- target: {} -- provider origin package: {} -- provider package key: {} -- service schema: {} -- service schema package: {} -- calling plan report fingerprint: {} -- calling plan commitment: {} -- selected: {} -- requirement owner: {} -- requirement owner package: {} -- requirement identity: {} -- method: {} -- subject: {} -- flow: {} -- domain: {} -- carry: {} -- predicate discharge: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_report_fingerprint,
                hex_bytes(row.provider_plan_digest.as_bytes()),
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                package_key_text(row.provider_type_package_identity),
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                package_key_text(row.provider_origin_package_identity),
                row.service_schema,
                package_key_text(row.service_schema_package_identity),
                row.calling_plan_report_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                row.calling_plan_commitment.map_or_else(
                    || "<none>".to_owned(),
                    |commitment| hex_bytes(&commitment.as_bytes()),
                ),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                package_key_text(row.requirement_owner_package_identity),
                row.requirement_identity,
                row.method,
                row.subject,
                row.authority_flow,
                row.domain,
                row.effective_carry,
                if row.predicate_discharge_required {
                    "required"
                } else {
                    "none"
                },
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        self.write_text("trust_report.md", &output)
    }
}

fn package_key_text(identity: Option<semantic_vocabulary::PackageKeyIdentity>) -> String {
    let Some(identity) = identity else {
        return "<unbound>".to_owned();
    };
    identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn progress_premises_text(premises: &[TrustProgressPremiseRow]) -> String {
    if premises.is_empty() {
        return "none".to_owned();
    }
    premises
        .iter()
        .map(|premise| {
            let mut subject = match premise.subject {
                TrustProgressPremiseSubject::ProviderReceiver => {
                    "provider-receiver(build-bound)".to_owned()
                }
                TrustProgressPremiseSubject::Parameter(index) => {
                    format!("parameter:{index}")
                }
            };
            for projection in &premise.subject_projections {
                subject.push('.');
                subject.push_str(projection);
            }
            format!("{}({subject})", premise.profile)
        })
        .collect::<Vec<_>>()
        .join(", ")
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
