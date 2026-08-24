pub use crate::identity::{AliasName, PackageName};
use crate::json::{JsonParseError, JsonParser, JsonValue};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub kind: String,
    pub locator: String,
    pub resolved: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyAlias {
    pub alias: AliasName,
    pub package: PackageName,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildMachineManifest {
    pub machine: Option<String>,
    pub service_reach: Vec<String>,
    pub observation_class: String,
    pub receipts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderRequirement {
    pub requirement: String,
    pub service_reach: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderSelection {
    pub requirement: String,
    pub provider: String,
    pub origin: String,
    pub plan_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QualificationRoute {
    pub kind: String,
    pub requirement: String,
    pub evidence_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapabilityFlowSummary {
    pub capability: String,
    pub verb: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstallationBoundReach {
    pub requirement: String,
    pub upper_bound: Vec<String>,
    pub resolved: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TrustReceipt {
    pub kind: String,
    pub subject: String,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReproducibilityEvidence {
    pub kind: String,
    pub verdict: String,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCapabilityManifest {
    pub schema_version: u32,
    pub package: PackageName,
    pub source: SourceIdentity,
    pub public_api_contract_identity: Option<String>,
    pub dependency_aliases: Vec<DependencyAlias>,
    pub exported_service_reach: Vec<String>,
    pub build_machine: BuildMachineManifest,
    pub provider_requirements: Vec<ProviderRequirement>,
    pub provider_selections: Vec<ProviderSelection>,
    pub routed_qualifications: Vec<QualificationRoute>,
    pub capability_flows: Vec<CapabilityFlowSummary>,
    pub unresolved_installation_reaches: Vec<InstallationBoundReach>,
    pub trust_receipts: Vec<TrustReceipt>,
    pub reproducibility: Vec<ReproducibilityEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCapabilityManifestParseError {
    InvalidJson { message: String },
    MissingField { field: String },
    UnexpectedField { field: String },
    InvalidField { field: String, message: String },
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCapabilityManifestPersistenceError {
    Io { path: PathBuf, message: String },
    Parse(PackageCapabilityManifestParseError),
}

impl PackageCapabilityManifest {
    pub fn new(package: PackageName, source: SourceIdentity) -> Self {
        Self {
            schema_version: PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION,
            package,
            source,
            public_api_contract_identity: None,
            dependency_aliases: Vec::new(),
            exported_service_reach: Vec::new(),
            build_machine: BuildMachineManifest {
                machine: None,
                service_reach: Vec::new(),
                observation_class: "none".to_owned(),
                receipts: Vec::new(),
            },
            provider_requirements: Vec::new(),
            provider_selections: Vec::new(),
            routed_qualifications: Vec::new(),
            capability_flows: Vec::new(),
            unresolved_installation_reaches: Vec::new(),
            trust_receipts: Vec::new(),
            reproducibility: Vec::new(),
        }
    }

    pub fn normalized(mut self) -> Self {
        self.exported_service_reach = sorted_unique(self.exported_service_reach);
        self.build_machine.service_reach = sorted_unique(self.build_machine.service_reach);
        self.build_machine.receipts = sorted_unique(self.build_machine.receipts);
        for requirement in &mut self.provider_requirements {
            requirement.service_reach =
                sorted_unique(std::mem::take(&mut requirement.service_reach));
        }
        for reach in &mut self.unresolved_installation_reaches {
            reach.upper_bound = sorted_unique(std::mem::take(&mut reach.upper_bound));
            reach.resolved = sorted_unique(std::mem::take(&mut reach.resolved));
        }
        self.dependency_aliases.sort();
        self.provider_requirements.sort();
        self.provider_selections.sort();
        self.routed_qualifications.sort();
        self.capability_flows.sort();
        self.unresolved_installation_reaches.sort();
        self.trust_receipts.sort();
        self.reproducibility.sort();
        self
    }

    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.normalized_clone().to_json().as_bytes());
        format_sha256(&hasher.finalize())
    }

    pub fn normalized_clone(&self) -> Self {
        self.clone().normalized()
    }

    pub fn from_json(json: &str) -> Result<Self, PackageCapabilityManifestParseError> {
        let value = JsonParser::new(json)
            .parse()
            .map_err(package_manifest_json_error)?;
        let root = value_object(&value, "$")?;
        ensure_fields(
            root,
            "$",
            &[
                "schema_version",
                "package",
                "source",
                "public_api_contract_identity",
                "dependency_aliases",
                "exported_service_reach",
                "build_machine",
                "provider_requirements",
                "provider_selections",
                "routed_qualifications",
                "capability_flows",
                "unresolved_installation_reaches",
                "trust_receipts",
                "reproducibility",
            ],
        )?;
        let schema_version = value_u32(field(root, "schema_version", "$")?, "$.schema_version")?;
        if schema_version != PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION {
            return Err(
                PackageCapabilityManifestParseError::UnsupportedSchemaVersion {
                    found: schema_version,
                    supported: PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION,
                },
            );
        }
        let mut manifest = Self {
            schema_version,
            package: parse_package_name(
                value_string(field(root, "package", "$")?, "$.package")?,
                "$.package",
            )?,
            source: parse_source_identity(field(root, "source", "$")?, "$.source")?,
            public_api_contract_identity: optional_string(
                field(root, "public_api_contract_identity", "$")?,
                "$.public_api_contract_identity",
            )?,
            dependency_aliases: parse_dependency_aliases(
                field(root, "dependency_aliases", "$")?,
                "$.dependency_aliases",
            )?,
            exported_service_reach: parse_string_array(
                field(root, "exported_service_reach", "$")?,
                "$.exported_service_reach",
            )?,
            build_machine: parse_build_machine(
                field(root, "build_machine", "$")?,
                "$.build_machine",
            )?,
            provider_requirements: parse_provider_requirements(
                field(root, "provider_requirements", "$")?,
                "$.provider_requirements",
            )?,
            provider_selections: parse_provider_selections(
                field(root, "provider_selections", "$")?,
                "$.provider_selections",
            )?,
            routed_qualifications: parse_qualification_routes(
                field(root, "routed_qualifications", "$")?,
                "$.routed_qualifications",
            )?,
            capability_flows: parse_capability_flows(
                field(root, "capability_flows", "$")?,
                "$.capability_flows",
            )?,
            unresolved_installation_reaches: parse_installation_reaches(
                field(root, "unresolved_installation_reaches", "$")?,
                "$.unresolved_installation_reaches",
            )?,
            trust_receipts: parse_trust_receipts(
                field(root, "trust_receipts", "$")?,
                "$.trust_receipts",
            )?,
            reproducibility: parse_reproducibility(
                field(root, "reproducibility", "$")?,
                "$.reproducibility",
            )?,
        };
        manifest = manifest.normalized();
        Ok(manifest)
    }

    pub fn read_from_path(
        path: impl AsRef<Path>,
    ) -> Result<Self, PackageCapabilityManifestPersistenceError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| {
            PackageCapabilityManifestPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            }
        })?;
        Self::from_json(&contents).map_err(PackageCapabilityManifestPersistenceError::Parse)
    }

    pub fn write_to_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), PackageCapabilityManifestPersistenceError> {
        let path = path.as_ref();
        let temp_path = temporary_manifest_path(path, self);
        fs::write(&temp_path, self.to_json()).map_err(|error| {
            PackageCapabilityManifestPersistenceError::Io {
                path: temp_path.clone(),
                message: error.to_string(),
            }
        })?;
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(PackageCapabilityManifestPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        let normalized = self.normalized_clone();
        let mut json = String::new();
        json.push_str("{\n");
        push_number_field(
            &mut json,
            1,
            "schema_version",
            normalized.schema_version,
            true,
        );
        push_string_field(&mut json, 1, "package", normalized.package.as_str(), true);
        push_object_start(&mut json, 1, "source");
        push_string_field(&mut json, 2, "kind", &normalized.source.kind, true);
        push_string_field(&mut json, 2, "locator", &normalized.source.locator, true);
        push_string_field(&mut json, 2, "resolved", &normalized.source.resolved, false);
        push_indent(&mut json, 1);
        json.push('}');
        json.push_str(",\n");
        push_optional_string_field(
            &mut json,
            1,
            "public_api_contract_identity",
            normalized.public_api_contract_identity.as_deref(),
            true,
        );
        push_dependency_aliases(&mut json, &normalized.dependency_aliases);
        push_string_array_field(
            &mut json,
            1,
            "exported_service_reach",
            &normalized.exported_service_reach,
            true,
        );
        push_build_machine(&mut json, &normalized.build_machine);
        push_provider_requirements(&mut json, &normalized.provider_requirements);
        push_provider_selections(&mut json, &normalized.provider_selections);
        push_qualification_routes(&mut json, &normalized.routed_qualifications);
        push_capability_flows(&mut json, &normalized.capability_flows);
        push_installation_reaches(&mut json, &normalized.unresolved_installation_reaches);
        push_trust_receipts(&mut json, &normalized.trust_receipts);
        push_reproducibility(&mut json, &normalized.reproducibility);
        json.push_str("\n}\n");
        json
    }
}

fn package_manifest_json_error(error: JsonParseError) -> PackageCapabilityManifestParseError {
    match error {
        JsonParseError::InvalidJson { message } => {
            PackageCapabilityManifestParseError::InvalidJson { message }
        }
    }
}

fn parse_source_identity(
    value: &JsonValue,
    path: &str,
) -> Result<SourceIdentity, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["kind", "locator", "resolved"])?;
    Ok(SourceIdentity {
        kind: value_string(field(fields, "kind", path)?, &format!("{path}.kind"))?.to_owned(),
        locator: value_string(field(fields, "locator", path)?, &format!("{path}.locator"))?
            .to_owned(),
        resolved: value_string(
            field(fields, "resolved", path)?,
            &format!("{path}.resolved"),
        )?
        .to_owned(),
    })
}

fn parse_dependency_aliases(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<DependencyAlias>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_dependency_alias(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_dependency_alias(
    value: &JsonValue,
    path: &str,
) -> Result<DependencyAlias, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["alias", "package", "source_fingerprint"])?;
    Ok(DependencyAlias {
        alias: parse_alias_name(
            value_string(field(fields, "alias", path)?, &format!("{path}.alias"))?,
            &format!("{path}.alias"),
        )?,
        package: parse_package_name(
            value_string(field(fields, "package", path)?, &format!("{path}.package"))?,
            &format!("{path}.package"),
        )?,
        source_fingerprint: value_string(
            field(fields, "source_fingerprint", path)?,
            &format!("{path}.source_fingerprint"),
        )?
        .to_owned(),
    })
}

fn parse_build_machine(
    value: &JsonValue,
    path: &str,
) -> Result<BuildMachineManifest, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(
        fields,
        path,
        &["machine", "service_reach", "observation_class", "receipts"],
    )?;
    Ok(BuildMachineManifest {
        machine: optional_string(field(fields, "machine", path)?, &format!("{path}.machine"))?,
        service_reach: parse_string_array(
            field(fields, "service_reach", path)?,
            &format!("{path}.service_reach"),
        )?,
        observation_class: value_string(
            field(fields, "observation_class", path)?,
            &format!("{path}.observation_class"),
        )?
        .to_owned(),
        receipts: parse_string_array(
            field(fields, "receipts", path)?,
            &format!("{path}.receipts"),
        )?,
    })
}

fn parse_provider_requirements(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<ProviderRequirement>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_provider_requirement(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_provider_requirement(
    value: &JsonValue,
    path: &str,
) -> Result<ProviderRequirement, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["requirement", "service_reach"])?;
    Ok(ProviderRequirement {
        requirement: value_string(
            field(fields, "requirement", path)?,
            &format!("{path}.requirement"),
        )?
        .to_owned(),
        service_reach: parse_string_array(
            field(fields, "service_reach", path)?,
            &format!("{path}.service_reach"),
        )?,
    })
}

fn parse_provider_selections(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<ProviderSelection>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_provider_selection(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_provider_selection(
    value: &JsonValue,
    path: &str,
) -> Result<ProviderSelection, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(
        fields,
        path,
        &["requirement", "provider", "origin", "plan_identity"],
    )?;
    Ok(ProviderSelection {
        requirement: value_string(
            field(fields, "requirement", path)?,
            &format!("{path}.requirement"),
        )?
        .to_owned(),
        provider: value_string(
            field(fields, "provider", path)?,
            &format!("{path}.provider"),
        )?
        .to_owned(),
        origin: value_string(field(fields, "origin", path)?, &format!("{path}.origin"))?.to_owned(),
        plan_identity: value_string(
            field(fields, "plan_identity", path)?,
            &format!("{path}.plan_identity"),
        )?
        .to_owned(),
    })
}

fn parse_qualification_routes(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<QualificationRoute>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_qualification_route(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_qualification_route(
    value: &JsonValue,
    path: &str,
) -> Result<QualificationRoute, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["kind", "requirement", "evidence_identity"])?;
    Ok(QualificationRoute {
        kind: value_string(field(fields, "kind", path)?, &format!("{path}.kind"))?.to_owned(),
        requirement: value_string(
            field(fields, "requirement", path)?,
            &format!("{path}.requirement"),
        )?
        .to_owned(),
        evidence_identity: value_string(
            field(fields, "evidence_identity", path)?,
            &format!("{path}.evidence_identity"),
        )?
        .to_owned(),
    })
}

fn parse_capability_flows(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<CapabilityFlowSummary>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_capability_flow(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_capability_flow(
    value: &JsonValue,
    path: &str,
) -> Result<CapabilityFlowSummary, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["capability", "verb", "count"])?;
    Ok(CapabilityFlowSummary {
        capability: value_string(
            field(fields, "capability", path)?,
            &format!("{path}.capability"),
        )?
        .to_owned(),
        verb: value_string(field(fields, "verb", path)?, &format!("{path}.verb"))?.to_owned(),
        count: value_u64(field(fields, "count", path)?, &format!("{path}.count"))?,
    })
}

fn parse_installation_reaches(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<InstallationBoundReach>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_installation_reach(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_installation_reach(
    value: &JsonValue,
    path: &str,
) -> Result<InstallationBoundReach, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["requirement", "upper_bound", "resolved"])?;
    Ok(InstallationBoundReach {
        requirement: value_string(
            field(fields, "requirement", path)?,
            &format!("{path}.requirement"),
        )?
        .to_owned(),
        upper_bound: parse_string_array(
            field(fields, "upper_bound", path)?,
            &format!("{path}.upper_bound"),
        )?,
        resolved: parse_string_array(
            field(fields, "resolved", path)?,
            &format!("{path}.resolved"),
        )?,
    })
}

fn parse_trust_receipts(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<TrustReceipt>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_trust_receipt(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_trust_receipt(
    value: &JsonValue,
    path: &str,
) -> Result<TrustReceipt, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["kind", "subject", "identity"])?;
    Ok(TrustReceipt {
        kind: value_string(field(fields, "kind", path)?, &format!("{path}.kind"))?.to_owned(),
        subject: value_string(field(fields, "subject", path)?, &format!("{path}.subject"))?
            .to_owned(),
        identity: value_string(
            field(fields, "identity", path)?,
            &format!("{path}.identity"),
        )?
        .to_owned(),
    })
}

fn parse_reproducibility(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<ReproducibilityEvidence>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_reproducibility_evidence(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_reproducibility_evidence(
    value: &JsonValue,
    path: &str,
) -> Result<ReproducibilityEvidence, PackageCapabilityManifestParseError> {
    let fields = value_object(value, path)?;
    ensure_fields(fields, path, &["kind", "verdict", "identity"])?;
    Ok(ReproducibilityEvidence {
        kind: value_string(field(fields, "kind", path)?, &format!("{path}.kind"))?.to_owned(),
        verdict: value_string(field(fields, "verdict", path)?, &format!("{path}.verdict"))?
            .to_owned(),
        identity: value_string(
            field(fields, "identity", path)?,
            &format!("{path}.identity"),
        )?
        .to_owned(),
    })
}

fn parse_string_array(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<String>, PackageCapabilityManifestParseError> {
    value_array(value, path)?
        .iter()
        .enumerate()
        .map(|(index, value)| value_string(value, &format!("{path}[{index}]")).map(str::to_owned))
        .collect()
}

fn ensure_fields(
    fields: &[(String, JsonValue)],
    path: &str,
    allowed: &[&str],
) -> Result<(), PackageCapabilityManifestParseError> {
    let actual = fields
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = allowed.iter().copied().collect::<BTreeSet<_>>();
    for name in actual.difference(&expected) {
        return Err(PackageCapabilityManifestParseError::UnexpectedField {
            field: format!("{path}.{name}"),
        });
    }
    for name in expected.difference(&actual) {
        return Err(PackageCapabilityManifestParseError::MissingField {
            field: format!("{path}.{name}"),
        });
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
    path: &str,
) -> Result<&'a JsonValue, PackageCapabilityManifestParseError> {
    fields
        .iter()
        .find(|(field_name, _)| field_name == name)
        .map(|(_, value)| value)
        .ok_or_else(|| PackageCapabilityManifestParseError::MissingField {
            field: format!("{path}.{name}"),
        })
}

fn value_object<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [(String, JsonValue)], PackageCapabilityManifestParseError> {
    value
        .as_object()
        .ok_or_else(|| PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON object".to_owned(),
        })
}

fn value_array<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a [JsonValue], PackageCapabilityManifestParseError> {
    value
        .as_array()
        .ok_or_else(|| PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON array".to_owned(),
        })
}

fn value_string<'a>(
    value: &'a JsonValue,
    path: &str,
) -> Result<&'a str, PackageCapabilityManifestParseError> {
    match value {
        JsonValue::String(value) => Ok(value),
        _ => Err(PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON string".to_owned(),
        }),
    }
}

fn optional_string(
    value: &JsonValue,
    path: &str,
) -> Result<Option<String>, PackageCapabilityManifestParseError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => Ok(Some(value.clone())),
        _ => Err(PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON string or null".to_owned(),
        }),
    }
}

fn value_u32(value: &JsonValue, path: &str) -> Result<u32, PackageCapabilityManifestParseError> {
    match value {
        JsonValue::Number(value) => u32::try_from(*value).map_err(|error| {
            PackageCapabilityManifestParseError::InvalidField {
                field: path.to_owned(),
                message: error.to_string(),
            }
        }),
        _ => Err(PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON integer".to_owned(),
        }),
    }
}

fn value_u64(value: &JsonValue, path: &str) -> Result<u64, PackageCapabilityManifestParseError> {
    match value {
        JsonValue::Number(value) => Ok(*value),
        _ => Err(PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON integer".to_owned(),
        }),
    }
}

fn parse_package_name(
    value: &str,
    path: &str,
) -> Result<PackageName, PackageCapabilityManifestParseError> {
    PackageName::parse(value).map_err(
        |message| PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message,
        },
    )
}

fn parse_alias_name(
    value: &str,
    path: &str,
) -> Result<AliasName, PackageCapabilityManifestParseError> {
    AliasName::parse(value).map_err(
        |message| PackageCapabilityManifestParseError::InvalidField {
            field: path.to_owned(),
            message,
        },
    )
}

fn temporary_manifest_path(path: &Path, manifest: &PackageCapabilityManifest) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega.package-manifest.json");
    let temp_name = format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        manifest.fingerprint()
    );
    path.with_file_name(temp_name)
}

pub(crate) fn section_json(manifest: &PackageCapabilityManifest, section: &str) -> String {
    let manifest = manifest.normalized_clone();
    match section {
        "source" => {
            let mut json = String::new();
            json.push('{');
            push_inline_string_field(&mut json, "kind", &manifest.source.kind, true);
            push_inline_string_field(&mut json, "locator", &manifest.source.locator, true);
            push_inline_string_field(&mut json, "resolved", &manifest.source.resolved, false);
            json.push('}');
            json
        }
        "public_api_contract_identity" => manifest.public_api_contract_identity.unwrap_or_default(),
        "dependency_aliases" => format!("{:?}", manifest.dependency_aliases),
        "exported_service_reach" => format!("{:?}", manifest.exported_service_reach),
        "build_machine" => format!("{:?}", manifest.build_machine),
        "provider_requirements" => format!("{:?}", manifest.provider_requirements),
        "provider_selections" => format!("{:?}", manifest.provider_selections),
        "routed_qualifications" => format!("{:?}", manifest.routed_qualifications),
        "capability_flows" => format!("{:?}", manifest.capability_flows),
        "unresolved_installation_reaches" => {
            format!("{:?}", manifest.unresolved_installation_reaches)
        }
        "trust_receipts" => format!("{:?}", manifest.trust_receipts),
        "reproducibility" => format!("{:?}", manifest.reproducibility),
        _ => String::new(),
    }
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn format_sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn push_dependency_aliases(json: &mut String, aliases: &[DependencyAlias]) {
    push_array_start(json, 1, "dependency_aliases");
    for (index, alias) in aliases.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "alias", alias.alias.as_str(), true);
        push_inline_string_field(json, "package", alias.package.as_str(), true);
        push_inline_string_field(json, "source_fingerprint", &alias.source_fingerprint, false);
        json.push('}');
    }
    push_array_end(json, 1, !aliases.is_empty(), true);
}

fn push_build_machine(json: &mut String, build: &BuildMachineManifest) {
    push_object_start(json, 1, "build_machine");
    push_optional_string_field(json, 2, "machine", build.machine.as_deref(), true);
    push_string_array_field(json, 2, "service_reach", &build.service_reach, true);
    push_string_field(json, 2, "observation_class", &build.observation_class, true);
    push_string_array_field(json, 2, "receipts", &build.receipts, false);
    push_indent(json, 1);
    json.push('}');
    json.push_str(",\n");
}

fn push_provider_requirements(json: &mut String, requirements: &[ProviderRequirement]) {
    push_array_start(json, 1, "provider_requirements");
    for (index, requirement) in requirements.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "requirement", &requirement.requirement, true);
        json.push_str("\"service_reach\": ");
        push_inline_string_array(json, &requirement.service_reach);
        json.push('}');
    }
    push_array_end(json, 1, !requirements.is_empty(), true);
}

fn push_provider_selections(json: &mut String, selections: &[ProviderSelection]) {
    push_array_start(json, 1, "provider_selections");
    for (index, selection) in selections.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "requirement", &selection.requirement, true);
        push_inline_string_field(json, "provider", &selection.provider, true);
        push_inline_string_field(json, "origin", &selection.origin, true);
        push_inline_string_field(json, "plan_identity", &selection.plan_identity, false);
        json.push('}');
    }
    push_array_end(json, 1, !selections.is_empty(), true);
}

fn push_qualification_routes(json: &mut String, routes: &[QualificationRoute]) {
    push_array_start(json, 1, "routed_qualifications");
    for (index, route) in routes.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "kind", &route.kind, true);
        push_inline_string_field(json, "requirement", &route.requirement, true);
        push_inline_string_field(json, "evidence_identity", &route.evidence_identity, false);
        json.push('}');
    }
    push_array_end(json, 1, !routes.is_empty(), true);
}

fn push_capability_flows(json: &mut String, flows: &[CapabilityFlowSummary]) {
    push_array_start(json, 1, "capability_flows");
    for (index, flow) in flows.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "capability", &flow.capability, true);
        push_inline_string_field(json, "verb", &flow.verb, true);
        json.push_str("\"count\": ");
        json.push_str(&flow.count.to_string());
        json.push('}');
    }
    push_array_end(json, 1, !flows.is_empty(), true);
}

fn push_installation_reaches(json: &mut String, reaches: &[InstallationBoundReach]) {
    push_array_start(json, 1, "unresolved_installation_reaches");
    for (index, reach) in reaches.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "requirement", &reach.requirement, true);
        json.push_str("\"upper_bound\": ");
        push_inline_string_array(json, &reach.upper_bound);
        json.push_str(", \"resolved\": ");
        push_inline_string_array(json, &reach.resolved);
        json.push('}');
    }
    push_array_end(json, 1, !reaches.is_empty(), true);
}

fn push_trust_receipts(json: &mut String, receipts: &[TrustReceipt]) {
    push_array_start(json, 1, "trust_receipts");
    for (index, receipt) in receipts.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "kind", &receipt.kind, true);
        push_inline_string_field(json, "subject", &receipt.subject, true);
        push_inline_string_field(json, "identity", &receipt.identity, false);
        json.push('}');
    }
    push_array_end(json, 1, !receipts.is_empty(), true);
}

fn push_reproducibility(json: &mut String, evidence: &[ReproducibilityEvidence]) {
    push_array_start(json, 1, "reproducibility");
    for (index, row) in evidence.iter().enumerate() {
        push_array_object_prefix(json, 2, index);
        push_inline_string_field(json, "kind", &row.kind, true);
        push_inline_string_field(json, "verdict", &row.verdict, true);
        push_inline_string_field(json, "identity", &row.identity, false);
        json.push('}');
    }
    push_array_end(json, 1, !evidence.is_empty(), false);
}

fn push_object_start(json: &mut String, indent: usize, name: &str) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": {\n");
}

fn push_array_start(json: &mut String, indent: usize, name: &str) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": [");
}

fn push_array_end(json: &mut String, indent: usize, multiline: bool, comma_after: bool) {
    if multiline {
        json.push('\n');
        push_indent(json, indent);
    }
    json.push(']');
    if comma_after {
        json.push(',');
    }
    json.push('\n');
}

fn push_array_object_prefix(json: &mut String, indent: usize, index: usize) {
    if index == 0 {
        json.push('\n');
    } else {
        json.push_str(",\n");
    }
    push_indent(json, indent);
    json.push('{');
}

fn push_string_field(json: &mut String, indent: usize, name: &str, value: &str, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_optional_string_field(
    json: &mut String,
    indent: usize,
    name: &str,
    value: Option<&str>,
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    if let Some(value) = value {
        push_json_string(json, value);
    } else {
        json.push_str("null");
    }
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_number_field(json: &mut String, indent: usize, name: &str, value: u32, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_string_array_field(
    json: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    push_inline_string_array(json, values);
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_inline_string_field(json: &mut String, name: &str, value: &str, comma: bool) {
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push_str(", ");
    }
}

fn push_inline_string_array(json: &mut String, values: &[String]) {
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
    json.push(']');
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => {
                json.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => json.push(ch),
        }
    }
    json.push('"');
}

fn push_indent(json: &mut String, level: usize) {
    for _ in 0..level {
        json.push_str("  ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-package-manifest-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn source() -> SourceIdentity {
        SourceIdentity {
            kind: "git".to_owned(),
            locator: "https://github.com/CathedralOS/generated-table".to_owned(),
            resolved: "commit:012345".to_owned(),
        }
    }

    fn full_manifest() -> PackageCapabilityManifest {
        let mut manifest = PackageCapabilityManifest::new(
            PackageName::parse("generated-table").unwrap(),
            source(),
        );
        manifest.public_api_contract_identity = Some("api:generated-table".to_owned());
        manifest.dependency_aliases.push(DependencyAlias {
            alias: AliasName::parse("file_journal").unwrap(),
            package: PackageName::parse("file-journal").unwrap(),
            source_fingerprint: "source:file-journal".to_owned(),
        });
        manifest.exported_service_reach = vec!["FilesystemHost".to_owned(), "Console".to_owned()];
        manifest.build_machine.machine = Some("Build::main".to_owned());
        manifest.build_machine.service_reach = vec!["FilesystemHost".to_owned()];
        manifest.build_machine.observation_class = "receipted".to_owned();
        manifest.build_machine.receipts = vec!["receipt:build-inputs".to_owned()];
        manifest.provider_requirements.push(ProviderRequirement {
            requirement: "FileJournalProvider".to_owned(),
            service_reach: vec!["FilesystemHost".to_owned()],
        });
        manifest.provider_selections.push(ProviderSelection {
            requirement: "FileJournalProvider".to_owned(),
            provider: "HostFileJournal".to_owned(),
            origin: "root-build".to_owned(),
            plan_identity: "plan:file-journal".to_owned(),
        });
        manifest.routed_qualifications.push(QualificationRoute {
            kind: "provider".to_owned(),
            requirement: "FileJournalProvider".to_owned(),
            evidence_identity: "route:file-journal".to_owned(),
        });
        manifest.capability_flows.push(CapabilityFlowSummary {
            capability: "File".to_owned(),
            verb: "returns".to_owned(),
            count: 1,
        });
        manifest
            .unresolved_installation_reaches
            .push(InstallationBoundReach {
                requirement: "JournalBackend".to_owned(),
                upper_bound: vec!["FilesystemHost".to_owned()],
                resolved: vec!["FilesystemHost".to_owned()],
            });
        manifest.trust_receipts.push(TrustReceipt {
            kind: "review".to_owned(),
            subject: "filesystem".to_owned(),
            identity: "receipt:filesystem".to_owned(),
        });
        manifest.reproducibility.push(ReproducibilityEvidence {
            kind: "source-cache".to_owned(),
            verdict: "accepted".to_owned(),
            identity: "source-cache:generated-table".to_owned(),
        });
        manifest
    }

    #[test]
    fn package_names_are_kebab_case_and_aliases_are_snake_case() {
        assert!(PackageName::parse("generated-table").is_ok());
        assert!(PackageName::parse("generated_table").is_err());
        assert!(AliasName::parse("generated_table").is_ok());
        assert!(AliasName::parse("generated-table").is_err());
    }

    #[test]
    fn manifest_json_and_fingerprint_are_normalized() {
        let mut left = PackageCapabilityManifest::new(
            PackageName::parse("generated-table").unwrap(),
            source(),
        );
        left.exported_service_reach = vec!["FilesystemHost".to_owned(), "Console".to_owned()];
        left.capability_flows = vec![
            CapabilityFlowSummary {
                capability: "File".to_owned(),
                verb: "returns".to_owned(),
                count: 1,
            },
            CapabilityFlowSummary {
                capability: "File".to_owned(),
                verb: "stores".to_owned(),
                count: 2,
            },
        ];

        let mut right = PackageCapabilityManifest::new(
            PackageName::parse("generated-table").unwrap(),
            source(),
        );
        right.capability_flows = left.capability_flows.iter().cloned().rev().collect();
        right.exported_service_reach = vec!["Console".to_owned(), "FilesystemHost".to_owned()];

        assert_eq!(left.to_json(), right.to_json());
        assert_eq!(left.fingerprint(), right.fingerprint());
    }

    #[test]
    fn manifest_json_parse_round_trip_is_normalized() {
        let manifest = full_manifest();

        let parsed =
            PackageCapabilityManifest::from_json(&manifest.to_json()).expect("parse manifest JSON");

        assert_eq!(parsed.to_json(), manifest.to_json());
        assert_eq!(parsed.fingerprint(), manifest.fingerprint());
    }

    #[test]
    fn manifest_read_write_round_trip_uses_strict_parser() {
        let manifest = full_manifest();
        let root = temp_root("read-write");
        let path = root.join("package-manifest.json");
        std::fs::create_dir_all(&root).expect("create manifest dir");

        manifest.write_to_path(&path).expect("write manifest");
        let read = PackageCapabilityManifest::read_from_path(&path).expect("read manifest");

        assert_eq!(read.to_json(), manifest.to_json());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn manifest_parse_rejects_unknown_schema_or_bad_package_name() {
        let mut json = full_manifest().to_json();
        json = json.replacen("\"schema_version\": 1", "\"schema_version\": 99", 1);
        assert_eq!(
            PackageCapabilityManifest::from_json(&json),
            Err(
                PackageCapabilityManifestParseError::UnsupportedSchemaVersion {
                    found: 99,
                    supported: PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION,
                }
            )
        );

        let bad_package = full_manifest().to_json().replacen(
            "\"package\": \"generated-table\"",
            "\"package\": \"generated_table\"",
            1,
        );
        assert!(matches!(
            PackageCapabilityManifest::from_json(&bad_package),
            Err(PackageCapabilityManifestParseError::InvalidField { field, .. })
                if field == "$.package"
        ));
    }
}
