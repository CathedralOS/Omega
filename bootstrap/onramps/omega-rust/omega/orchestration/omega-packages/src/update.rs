use crate::audit::{PackageGraphAuditError, audit_package_graph};
use crate::diff::{ManifestDelta, ManifestDiff, diff_package_capability_manifests};
use crate::lock::{PackageLock, PackageLockAssemblyError};
use crate::manifest::{PackageCapabilityManifest, PackageName};
use crate::review::CapabilityChangeReceipt;

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
    AdmitReviewedChange {
        package: String,
        old_source_identity: String,
        new_source_identity: String,
        old_manifest_fingerprint: String,
        new_manifest_fingerprint: String,
        receipt_fingerprint: String,
        diff: ManifestDiff,
        blocking_deltas: Vec<ManifestDelta>,
    },
    RejectReceiptMismatch {
        package: String,
        receipt_fingerprint: String,
        diff: ManifestDiff,
        blocking_deltas: Vec<ManifestDelta>,
    },
}

impl PackageUpdateDecision {
    pub fn is_admitted(&self) -> bool {
        matches!(
            self,
            Self::AdmitSourceOnly { .. } | Self::AdmitReviewedChange { .. }
        )
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
            Self::AdmitReviewedChange {
                package,
                old_source_identity,
                new_source_identity,
                old_manifest_fingerprint,
                new_manifest_fingerprint,
                receipt_fingerprint,
                diff,
                blocking_deltas,
            } => {
                let mut report = String::new();
                report.push_str("package update admitted by capability-change receipt\n");
                report.push_str("package: ");
                report.push_str(package);
                report.push('\n');
                report.push_str("receipt: ");
                report.push_str(receipt_fingerprint);
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
                report.push_str("accepted sections:");
                for delta in blocking_deltas {
                    report.push(' ');
                    report.push_str(&delta.section);
                    report.push('(');
                    report.push_str(delta.severity.as_str());
                    report.push(')');
                }
                report.push('\n');
                report.push_str(&diff.to_text());
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
            Self::RejectReceiptMismatch {
                package,
                receipt_fingerprint,
                diff,
                blocking_deltas,
            } => {
                let mut report = String::new();
                report.push_str("package update rejected: capability-change receipt mismatch\n");
                report.push_str("package: ");
                report.push_str(package);
                report.push('\n');
                report.push_str("receipt: ");
                report.push_str(receipt_fingerprint);
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
                report.push_str("review required: create a receipt bound to this exact source pair, manifest fingerprints, and delta fingerprints\n");
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockUpdatePlan {
    pub target_package: String,
    pub current_lock_fingerprint: String,
    pub decision: PackageUpdateDecision,
    pub candidate_lock: Option<PackageLock>,
}

impl PackageLockUpdatePlan {
    pub fn is_admitted(&self) -> bool {
        self.decision.is_admitted()
    }

    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package lock update plan\n");
        report.push_str("target: ");
        report.push_str(&self.target_package);
        report.push('\n');
        report.push_str("current lock: ");
        report.push_str(&self.current_lock_fingerprint);
        report.push('\n');
        if let Some(lock) = &self.candidate_lock {
            report.push_str("candidate lock: ");
            report.push_str(&lock.fingerprint());
            report.push('\n');
        } else {
            report.push_str("candidate lock: none\n");
        }
        report.push_str(&self.decision.to_text());
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockUpdatePlanError {
    CurrentGraph(PackageGraphAuditError),
    MissingCurrentManifest { package: String },
    MissingCandidateManifest { package: String },
    TargetNotLocked { package: String },
    Admission(PackageUpdateAdmissionError),
    CandidateLock(PackageLockAssemblyError),
    CandidateGraph(PackageGraphAuditError),
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

pub fn plan_package_lock_update(
    current_lock: &PackageLock,
    current_manifests: &[PackageCapabilityManifest],
    candidate_manifests: &[PackageCapabilityManifest],
    target_package: &PackageName,
    receipt: Option<&CapabilityChangeReceipt>,
) -> Result<PackageLockUpdatePlan, PackageLockUpdatePlanError> {
    audit_package_graph(current_lock, current_manifests)
        .map_err(PackageLockUpdatePlanError::CurrentGraph)?;
    if current_lock.package(target_package).is_none() {
        return Err(PackageLockUpdatePlanError::TargetNotLocked {
            package: target_package.as_str().to_owned(),
        });
    }
    let current_manifest = manifest_for(current_manifests, target_package).ok_or_else(|| {
        PackageLockUpdatePlanError::MissingCurrentManifest {
            package: target_package.as_str().to_owned(),
        }
    })?;
    let candidate_manifest =
        manifest_for(candidate_manifests, target_package).ok_or_else(|| {
            PackageLockUpdatePlanError::MissingCandidateManifest {
                package: target_package.as_str().to_owned(),
            }
        })?;

    let decision = if let Some(receipt) = receipt {
        decide_reviewed_package_update(current_manifest, candidate_manifest, receipt)
    } else {
        decide_default_package_update(current_manifest, candidate_manifest)
    }
    .map_err(PackageLockUpdatePlanError::Admission)?;

    let candidate_lock = if decision.is_admitted() {
        let candidate_lock =
            PackageLock::from_manifests(current_lock.root_package.clone(), candidate_manifests)
                .map_err(PackageLockUpdatePlanError::CandidateLock)?;
        audit_package_graph(&candidate_lock, candidate_manifests)
            .map_err(PackageLockUpdatePlanError::CandidateGraph)?;
        Some(candidate_lock)
    } else {
        None
    };

    Ok(PackageLockUpdatePlan {
        target_package: target_package.as_str().to_owned(),
        current_lock_fingerprint: current_lock.fingerprint(),
        decision,
        candidate_lock,
    })
}

fn manifest_for<'a>(
    manifests: &'a [PackageCapabilityManifest],
    package: &PackageName,
) -> Option<&'a PackageCapabilityManifest> {
    manifests
        .iter()
        .find(|manifest| &manifest.package == package)
}

pub fn decide_reviewed_package_update(
    old: &PackageCapabilityManifest,
    new: &PackageCapabilityManifest,
    receipt: &CapabilityChangeReceipt,
) -> Result<PackageUpdateDecision, PackageUpdateAdmissionError> {
    let old = old.normalized_clone();
    let new = new.normalized_clone();
    let default_decision = decide_default_package_update(&old, &new)?;
    let PackageUpdateDecision::RejectManifestChange {
        package,
        diff,
        blocking_deltas,
    } = default_decision
    else {
        return Ok(default_decision);
    };

    let receipt_fingerprint = receipt.fingerprint();
    if receipt.accepts(&diff)
        && receipt.old_source_identity == old.source.resolved
        && receipt.new_source_identity == new.source.resolved
    {
        Ok(PackageUpdateDecision::AdmitReviewedChange {
            package,
            old_source_identity: old.source.resolved,
            new_source_identity: new.source.resolved,
            old_manifest_fingerprint: diff.old_manifest_fingerprint.clone(),
            new_manifest_fingerprint: diff.new_manifest_fingerprint.clone(),
            receipt_fingerprint,
            diff,
            blocking_deltas,
        })
    } else {
        Ok(PackageUpdateDecision::RejectReceiptMismatch {
            package,
            receipt_fingerprint,
            diff,
            blocking_deltas,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::{LockedDependency, LockedPackage};
    use crate::manifest::{AliasName, DependencyAlias, PackageName, SourceIdentity};
    use crate::review::CapabilityChangeReceipt;

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

    fn alias(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn package(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn locked_package(manifest: &PackageCapabilityManifest) -> LockedPackage {
        LockedPackage {
            package: manifest.package.clone(),
            source_kind: manifest.source.kind.clone(),
            source_locator: manifest.source.locator.clone(),
            source_identity: manifest.source.resolved.clone(),
            manifest_fingerprint: manifest.fingerprint(),
            build_observation: manifest.build_machine.observation_class.clone(),
            dependencies: Vec::new(),
            trust_receipts: Vec::new(),
        }
    }

    fn root_and_child_manifests(
        child: PackageCapabilityManifest,
    ) -> (PackageLock, Vec<PackageCapabilityManifest>) {
        let mut root_manifest = manifest("graph-workbench", "commit:root");
        root_manifest.dependency_aliases.push(DependencyAlias {
            alias: alias("file_journal"),
            package: package("file-journal"),
            source_fingerprint: "source:file-journal".to_owned(),
        });
        let mut root = locked_package(&root_manifest);
        root.dependencies.push(LockedDependency {
            alias: alias("file_journal"),
            package: package("file-journal"),
        });
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![root, locked_package(&child)];
        (lock, vec![root_manifest, child])
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
            PackageUpdateDecision::AdmitReviewedChange { .. } => {
                panic!("capability change admitted without receipt")
            }
            PackageUpdateDecision::RejectReceiptMismatch { .. } => {
                panic!("receipt mismatch without receipt")
            }
        }
    }

    #[test]
    fn reviewed_update_admits_exact_receipt_match() {
        let old = manifest("file-journal", "commit:old");
        let mut new = manifest("file-journal", "commit:new");
        new.exported_service_reach.push("FilesystemHost".to_owned());
        let diff = diff_package_capability_manifests(&old, &new);
        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            old.source.resolved.clone(),
            new.source.resolved.clone(),
            "reviewer@example.invalid",
            "audited filesystem reach",
        )
        .expect("receipt should be valid");

        let decision = decide_reviewed_package_update(&old, &new, &receipt)
            .expect("same package should decide");

        assert!(decision.is_admitted());
        assert!(matches!(
            decision,
            PackageUpdateDecision::AdmitReviewedChange { .. }
        ));
        assert!(
            decision
                .to_text()
                .contains("admitted by capability-change receipt")
        );
    }

    #[test]
    fn reviewed_update_rejects_receipt_for_different_diff() {
        let old = manifest("file-journal", "commit:old");
        let mut reviewed = manifest("file-journal", "commit:new");
        reviewed
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let reviewed_diff = diff_package_capability_manifests(&old, &reviewed);
        let receipt = CapabilityChangeReceipt::from_diff(
            &reviewed_diff,
            old.source.resolved.clone(),
            reviewed.source.resolved.clone(),
            "reviewer@example.invalid",
            "audited filesystem reach",
        )
        .expect("receipt should be valid");

        let mut actual = reviewed.clone();
        actual.exported_service_reach.push("NetworkHost".to_owned());
        let decision = decide_reviewed_package_update(&old, &actual, &receipt)
            .expect("same package should decide");

        assert!(!decision.is_admitted());
        assert!(matches!(
            decision,
            PackageUpdateDecision::RejectReceiptMismatch { .. }
        ));
        assert!(decision.to_text().contains("receipt mismatch"));
    }

    #[test]
    fn lock_update_plan_admits_source_only_candidate_lock() {
        let current_child = manifest("file-journal", "commit:old");
        let (current_lock, current_manifests) = root_and_child_manifests(current_child);
        let candidate_child = manifest("file-journal", "commit:new");
        let (_, candidate_manifests) = root_and_child_manifests(candidate_child);

        let plan = plan_package_lock_update(
            &current_lock,
            &current_manifests,
            &candidate_manifests,
            &package("file-journal"),
            None,
        )
        .expect("source-only update should plan");

        assert!(plan.is_admitted());
        let candidate_lock = plan
            .candidate_lock
            .as_ref()
            .expect("admitted plan should include candidate lock");
        let target = candidate_lock
            .package(&package("file-journal"))
            .expect("candidate lock should contain target");
        assert_eq!(target.source_identity, "commit:new");
        assert!(plan.to_text().contains("candidate lock: "));
    }

    #[test]
    fn lock_update_plan_rejects_manifest_change_without_candidate_lock() {
        let current_child = manifest("file-journal", "commit:old");
        let (current_lock, current_manifests) = root_and_child_manifests(current_child);
        let mut candidate_child = manifest("file-journal", "commit:new");
        candidate_child
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let (_, candidate_manifests) = root_and_child_manifests(candidate_child);

        let plan = plan_package_lock_update(
            &current_lock,
            &current_manifests,
            &candidate_manifests,
            &package("file-journal"),
            None,
        )
        .expect("capability-changing update should produce rejection plan");

        assert!(!plan.is_admitted());
        assert!(plan.candidate_lock.is_none());
        assert!(matches!(
            plan.decision,
            PackageUpdateDecision::RejectManifestChange { .. }
        ));
    }

    #[test]
    fn lock_update_plan_rejects_unreachable_candidate_package() {
        let current_child = manifest("file-journal", "commit:old");
        let (current_lock, current_manifests) = root_and_child_manifests(current_child);
        let candidate_child = manifest("file-journal", "commit:new");
        let (_, mut candidate_manifests) = root_and_child_manifests(candidate_child);
        candidate_manifests.push(manifest("arithmetic-kernels", "commit:math"));

        assert_eq!(
            plan_package_lock_update(
                &current_lock,
                &current_manifests,
                &candidate_manifests,
                &package("file-journal"),
                None,
            ),
            Err(PackageLockUpdatePlanError::CandidateGraph(
                PackageGraphAuditError::UnreachablePackage {
                    package: "arithmetic-kernels".to_owned(),
                }
            ))
        );
    }

    #[test]
    fn lock_update_plan_admits_exact_review_receipt() {
        let current_child = manifest("file-journal", "commit:old");
        let (current_lock, current_manifests) = root_and_child_manifests(current_child.clone());
        let mut candidate_child = manifest("file-journal", "commit:new");
        candidate_child
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let (_, candidate_manifests) = root_and_child_manifests(candidate_child.clone());
        let diff = diff_package_capability_manifests(&current_child, &candidate_child);
        let receipt = CapabilityChangeReceipt::from_diff(
            &diff,
            current_child.source.resolved,
            candidate_child.source.resolved,
            "reviewer@example.invalid",
            "audited filesystem reach",
        )
        .expect("receipt should be valid");

        let plan = plan_package_lock_update(
            &current_lock,
            &current_manifests,
            &candidate_manifests,
            &package("file-journal"),
            Some(&receipt),
        )
        .expect("reviewed update should plan");

        assert!(plan.is_admitted());
        assert!(plan.candidate_lock.is_some());
        assert!(matches!(
            plan.decision,
            PackageUpdateDecision::AdmitReviewedChange { .. }
        ));
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
