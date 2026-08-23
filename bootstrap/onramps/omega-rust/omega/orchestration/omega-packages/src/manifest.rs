use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PACKAGE_CAPABILITY_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageName(String);

impl PackageName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_kebab_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "package identity `{value}` must use kebab-case lowercase words"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AliasName(String);

impl AliasName {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if is_snake_case(&value) {
            Ok(Self(value))
        } else {
            Err(format!(
                "dependency alias `{value}` must use snake_case Omega identifier spelling"
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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

fn is_kebab_case(value: &str) -> bool {
    is_separated_lowercase(value, '-')
}

fn is_snake_case(value: &str) -> bool {
    is_separated_lowercase(value, '_')
}

fn is_separated_lowercase(value: &str, separator: char) -> bool {
    if value.is_empty() || value.starts_with(separator) || value.ends_with(separator) {
        return false;
    }
    let mut previous_separator = false;
    for ch in value.chars() {
        if ch == separator {
            if previous_separator {
                return false;
            }
            previous_separator = true;
            continue;
        }
        previous_separator = false;
        if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() {
            return false;
        }
    }
    true
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

    fn source() -> SourceIdentity {
        SourceIdentity {
            kind: "git".to_owned(),
            locator: "https://github.com/CathedralOS/generated-table".to_owned(),
            resolved: "commit:012345".to_owned(),
        }
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
}
