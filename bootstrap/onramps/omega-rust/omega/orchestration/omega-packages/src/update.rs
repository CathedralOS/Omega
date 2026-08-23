use crate::diff::{ManifestDelta, ManifestDiff, diff_package_capability_manifests};
use crate::manifest::PackageCapabilityManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageUpdateDecision {
    AdmitSourceOnly {
        package: String,
        old_source_identity: String,
        new_source_identity: String,
        old_manifest_fingerprint: String,
        new_manifest_fingerprint: String,
        source_deltas: Vec<ManifestDelta>,
    },
    RejectManifestChange {
        package: String,
        diff: ManifestDiff,
        blocking_deltas: Vec<ManifestDelta>,
    },
}

impl PackageUpdateDecision {
    pub fn is_admitted(&self) -> bool {
        matches!(self, Self::AdmitSourceOnly { .. })
    }

    pub fn to_text(&self) -> String {
        match self {
            Self::AdmitSourceOnly {
                package,
                old_source_identity,
                new_source_identity,
                old_manifest_fingerprint,
                new_manifest_fingerprint,
                source_deltas,
            } => {
                let mut report = String::new();
                report.push_str("package update admitted\n");
                report.push_str("package: ");
                report.push_str(package);
                report.push('\n');
                report.push_str("old source: ");
                report.push_str(old_source_identity);
                report.push('\n');
                report.push_str("new source: ");
                report.push_str(new_source_identity);
                report.push('\n');
                report.push_str("old manifest: ");
                report.push_str(old_manifest_fingerprint);
                report.push('\n');
                report.push_str("new manifest: ");
                report.push_str(new_manifest_fingerprint);
                report.push('\n');
                if source_deltas.is_empty() {
                    report.push_str("capability evidence: unchanged\n");
                } else {
                    report.push_str("capability evidence: unchanged except source identity\n");
                }
                report
            }
            Self::RejectManifestChange {
                package,
                diff,
                blocking_deltas,
            } => {
                let mut report = String::new();
                report.push_str("package update rejected: capability manifest changed\n");
                report.push_str("package: ");
                report.push_str(package);
                report.push('\n');
                report.push_str("blocking sections:");
                for delta in blocking_deltas {
                    report.push(' ');
                    report.push_str(&delta.section);
                    report.push('(');
                    report.push_str(delta.severity.as_str());
                    report.push(')');
                }
                report.push('\n');
                report.push_str(&diff.to_text());
                report.push_str("review required: rerun with an explicit capability-change acceptance receipt after auditing the changed sections\n");
                report
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageUpdateAdmissionError {
    PackageMismatch {
        old_package: String,
        new_package: String,
    },
}

pub fn decide_default_package_update(
    old: &PackageCapabilityManifest,
    new: &PackageCapabilityManifest,
) -> Result<PackageUpdateDecision, PackageUpdateAdmissionError> {
    let old = old.normalized_clone();
    let new = new.normalized_clone();
    if old.package != new.package {
        return Err(PackageUpdateAdmissionError::PackageMismatch {
            old_package: old.package.as_str().to_owned(),
            new_package: new.package.as_str().to_owned(),
        });
    }

    let diff = diff_package_capability_manifests(&old, &new);
    let (source_deltas, blocking_deltas): (Vec<_>, Vec<_>) = diff
        .deltas
        .iter()
        .cloned()
        .partition(|delta| delta.section == "source");

    if blocking_deltas.is_empty() {
        Ok(PackageUpdateDecision::AdmitSourceOnly {
            package: old.package.as_str().to_owned(),
            old_source_identity: old.source.resolved,
            new_source_identity: new.source.resolved,
            old_manifest_fingerprint: diff.old_manifest_fingerprint,
            new_manifest_fingerprint: diff.new_manifest_fingerprint,
            source_deltas,
        })
    } else {
        Ok(PackageUpdateDecision::RejectManifestChange {
            package: old.package.as_str().to_owned(),
            diff,
            blocking_deltas,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{PackageName, SourceIdentity};

    fn manifest(package: &str, resolved: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            PackageName::parse(package).unwrap(),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: resolved.to_owned(),
            },
        )
    }

    #[test]
    fn default_update_admits_unchanged_capability_manifest_with_new_source() {
        let old = manifest("arithmetic-kernels", "commit:old");
        let new = manifest("arithmetic-kernels", "commit:new");

        let decision =
            decide_default_package_update(&old, &new).expect("same package should decide");

        assert!(decision.is_admitted());
        assert!(matches!(
            decision,
            PackageUpdateDecision::AdmitSourceOnly { ref source_deltas, .. } if source_deltas.len() == 1
        ));
        assert!(
            decision
                .to_text()
                .contains("unchanged except source identity")
        );
    }

    #[test]
    fn default_update_rejects_public_capability_change() {
        let old = manifest("file-journal", "commit:old");
        let mut new = manifest("file-journal", "commit:new");
        new.exported_service_reach.push("FilesystemHost".to_owned());

        let decision =
            decide_default_package_update(&old, &new).expect("same package should decide");

        assert!(!decision.is_admitted());
        match decision {
            PackageUpdateDecision::RejectManifestChange {
                blocking_deltas, ..
            } => {
                assert!(
                    blocking_deltas
                        .iter()
                        .any(|delta| delta.section == "exported_service_reach")
                );
            }
            PackageUpdateDecision::AdmitSourceOnly { .. } => panic!("capability change admitted"),
        }
    }

    #[test]
    fn default_update_rejects_package_identity_change() {
        let old = manifest("arithmetic-kernels", "commit:old");
        let new = manifest("file-journal", "commit:new");

        assert_eq!(
            decide_default_package_update(&old, &new),
            Err(PackageUpdateAdmissionError::PackageMismatch {
                old_package: "arithmetic-kernels".to_owned(),
                new_package: "file-journal".to_owned(),
            })
        );
    }
}
