use crate::manifest::{PackageCapabilityManifest, section_json};
use sha2::{Digest, Sha256};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PackageName, SourceIdentity};

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
    }
}
