use crate::manifest::{CapabilityFlowSummary, PackageCapabilityManifest, section_json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManifestSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl ManifestSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDelta {
    pub section: String,
    pub severity: ManifestSeverity,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub guidance: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDiff {
    pub old_manifest_fingerprint: String,
    pub new_manifest_fingerprint: String,
    pub deltas: Vec<ManifestDelta>,
}

impl ManifestDiff {
    pub fn is_empty(&self) -> bool {
        self.deltas.is_empty()
    }

    pub fn to_text(&self) -> String {
        if self.is_empty() {
            return "package capability manifest unchanged\n".to_owned();
        }
        let mut report = String::new();
        report.push_str("package capability manifest changed\n");
        report.push_str("old: ");
        report.push_str(&self.old_manifest_fingerprint);
        report.push('\n');
        report.push_str("new: ");
        report.push_str(&self.new_manifest_fingerprint);
        report.push('\n');
        for delta in &self.deltas {
            report.push_str("- ");
            report.push_str(delta.severity.as_str());
            report.push_str(": ");
            report.push_str(&delta.section);
            report.push_str(" changed (");
            report.push_str(&delta.old_fingerprint);
            report.push_str(" -> ");
            report.push_str(&delta.new_fingerprint);
            report.push_str(")\n");
            for guidance in &delta.guidance {
                report.push_str("  guidance: ");
                report.push_str(guidance);
                report.push('\n');
            }
        }
        report
    }
}

pub fn diff_package_capability_manifests(
    old: &PackageCapabilityManifest,
    new: &PackageCapabilityManifest,
) -> ManifestDiff {
    const SECTIONS: &[(&str, ManifestSeverity)] = &[
        ("source", ManifestSeverity::Low),
        ("public_api_contract_identity", ManifestSeverity::Low),
        ("dependency_aliases", ManifestSeverity::Medium),
        ("exported_service_reach", ManifestSeverity::High),
        ("build_machine", ManifestSeverity::High),
        ("provider_requirements", ManifestSeverity::Medium),
        ("provider_selections", ManifestSeverity::High),
        ("routed_qualifications", ManifestSeverity::Medium),
        ("capability_flows", ManifestSeverity::Medium),
        ("unresolved_installation_reaches", ManifestSeverity::High),
        ("trust_receipts", ManifestSeverity::High),
        ("reproducibility", ManifestSeverity::Medium),
    ];

    let mut deltas = Vec::new();
    for (section, severity) in SECTIONS {
        let old_section = section_json(old, section);
        let new_section = section_json(new, section);
        if old_section != new_section {
            deltas.push(ManifestDelta {
                section: (*section).to_owned(),
                severity: *severity,
                old_fingerprint: fingerprint_bytes(old_section.as_bytes()),
                new_fingerprint: fingerprint_bytes(new_section.as_bytes()),
                guidance: audit_guidance_for_section(section, old, new),
            });
        }
    }

    deltas.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.section.cmp(&right.section))
    });

    ManifestDiff {
        old_manifest_fingerprint: old.fingerprint(),
        new_manifest_fingerprint: new.fingerprint(),
        deltas,
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn audit_guidance_for_section(
    section: &str,
    old: &PackageCapabilityManifest,
    new: &PackageCapabilityManifest,
) -> Vec<String> {
    let old = old.normalized_clone();
    let new = new.normalized_clone();
    match section {
        "exported_service_reach" => {
            added_strings(&old.exported_service_reach, &new.exported_service_reach)
                .into_iter()
                .map(|service| {
                    format!(
                        "new public service reach `{service}`; inspect the public boundary declarations and require dependent review"
                    )
                })
                .collect()
        }
        "build_machine" => {
            let mut guidance = added_strings(
                &old.build_machine.service_reach,
                &new.build_machine.service_reach,
            )
            .into_iter()
            .map(|service| {
                format!(
                    "new build-host service `{service}`; review build.omg first and confirm observations are receipted or intentionally volatile"
                )
            })
            .collect::<Vec<_>>();
            if old.build_machine.observation_class != new.build_machine.observation_class {
                guidance.push(format!(
                    "build observation class changed from `{}` to `{}`; verify replay evidence before accepting",
                    old.build_machine.observation_class, new.build_machine.observation_class
                ));
            }
            guidance
        }
        "provider_requirements" => added_strings(
            &old.provider_requirements
                .iter()
                .map(|requirement| requirement.requirement.clone())
                .collect::<Vec<_>>(),
            &new.provider_requirements
                .iter()
                .map(|requirement| requirement.requirement.clone())
                .collect::<Vec<_>>(),
        )
        .into_iter()
        .map(|requirement| {
            format!(
                "new provider requirement `{requirement}`; review provider origin, selected plan evidence, and whether the provider should live in a smaller optional package"
            )
        })
        .collect(),
        "capability_flows" => capability_flow_guidance(&old.capability_flows, &new.capability_flows),
        _ => Vec::new(),
    }
}

fn added_strings(old: &[String], new: &[String]) -> Vec<String> {
    let old = old.iter().collect::<BTreeSet<_>>();
    new.iter()
        .filter(|value| !old.contains(value))
        .cloned()
        .collect()
}

fn capability_flow_guidance(
    old: &[CapabilityFlowSummary],
    new: &[CapabilityFlowSummary],
) -> Vec<String> {
    let old_counts = capability_flow_counts(old);
    let new_counts = capability_flow_counts(new);
    new_counts
        .into_iter()
        .filter_map(|((capability, verb), new_count)| {
            let old_count = old_counts
                .get(&(capability.clone(), verb.clone()))
                .copied()
                .unwrap_or(0);
            (new_count > old_count).then(|| {
                format!(
                    "capability `{capability}` gained `{verb}` flow count {old_count}->{new_count}; inspect whether authority is stored, returned, acquired, or derived through the package API"
                )
            })
        })
        .collect()
}

fn capability_flow_counts(flows: &[CapabilityFlowSummary]) -> BTreeMap<(String, String), u64> {
    let mut counts = BTreeMap::new();
    for flow in flows {
        *counts
            .entry((flow.capability.clone(), flow.verb.clone()))
            .or_insert(0) += flow.count;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{
        BuildMachineManifest, CapabilityFlowSummary, PackageName, ProviderRequirement,
        SourceIdentity,
    };

    fn manifest(package: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            PackageName::parse(package).unwrap(),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: "commit:012345".to_owned(),
            },
        )
    }

    #[test]
    fn equal_manifests_have_no_delta() {
        let left = manifest("arithmetic-kernels");
        let right = manifest("arithmetic-kernels");
        let diff = diff_package_capability_manifests(&left, &right);
        assert!(diff.is_empty());
        assert_eq!(diff.to_text(), "package capability manifest unchanged\n");
    }

    #[test]
    fn service_reach_delta_is_high_severity() {
        let old = manifest("file-journal");
        let mut new = manifest("file-journal");
        new.exported_service_reach.push("FilesystemHost".to_owned());

        let diff = diff_package_capability_manifests(&old, &new);

        assert_eq!(diff.deltas.len(), 1);
        assert_eq!(diff.deltas[0].section, "exported_service_reach");
        assert_eq!(diff.deltas[0].severity, ManifestSeverity::High);
        assert!(diff.to_text().contains("high: exported_service_reach"));
        assert!(
            diff.to_text()
                .contains("new public service reach `FilesystemHost`")
        );
    }

    #[test]
    fn build_host_service_delta_explains_build_omg_audit() {
        let old = manifest("generated-table");
        let mut new = manifest("generated-table");
        new.build_machine = BuildMachineManifest {
            machine: Some("TableBuilder::build".to_owned()),
            service_reach: vec!["FilesystemHost".to_owned()],
            observation_class: "Receipted".to_owned(),
            receipts: vec!["build-fs-scope".to_owned()],
        };

        let diff = diff_package_capability_manifests(&old, &new);

        let build_delta = diff
            .deltas
            .iter()
            .find(|delta| delta.section == "build_machine")
            .expect("build machine delta");
        assert!(
            build_delta
                .guidance
                .iter()
                .any(|line| line.contains("review build.omg first"))
        );
    }

    #[test]
    fn provider_requirement_delta_recommends_provider_review_and_package_split() {
        let old = manifest("provider-switchboard");
        let mut new = manifest("provider-switchboard");
        new.provider_requirements.push(ProviderRequirement {
            requirement: "Clock::now".to_owned(),
            service_reach: vec!["ClockHost".to_owned()],
        });

        let diff = diff_package_capability_manifests(&old, &new);

        let provider_delta = diff
            .deltas
            .iter()
            .find(|delta| delta.section == "provider_requirements")
            .expect("provider requirement delta");
        assert!(
            provider_delta
                .guidance
                .iter()
                .any(|line| line.contains("provider origin"))
        );
        assert!(
            provider_delta
                .guidance
                .iter()
                .any(|line| line.contains("smaller optional package"))
        );
    }

    #[test]
    fn capability_flow_delta_explains_authority_flow_review() {
        let old = manifest("capability-vault");
        let mut new = manifest("capability-vault");
        new.capability_flows.push(CapabilityFlowSummary {
            capability: "FilesystemHandle".to_owned(),
            verb: "stores".to_owned(),
            count: 1,
        });

        let diff = diff_package_capability_manifests(&old, &new);

        let flow_delta = diff
            .deltas
            .iter()
            .find(|delta| delta.section == "capability_flows")
            .expect("capability flow delta");
        assert!(
            flow_delta
                .guidance
                .iter()
                .any(|line| line.contains("gained `stores` flow count 0->1"))
        );
    }
}
