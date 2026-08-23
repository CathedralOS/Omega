use crate::audit::{PackageGraphAudit, PackageGraphAuditError, audit_package_graph};
use crate::lock::{PackageLock, PackageLockAssemblyError};
use crate::manifest::{AliasName, PackageCapabilityManifest, PackageName};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInstallPlan {
    pub dependency_alias: String,
    pub dependency_package: String,
    pub current_lock_fingerprint: String,
    pub candidate_lock: PackageLock,
    pub candidate_audit: PackageGraphAudit,
    pub added_packages: Vec<String>,
}

impl PackageInstallPlan {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package install plan\n");
        report.push_str("alias: ");
        report.push_str(&self.dependency_alias);
        report.push('\n');
        report.push_str("package: ");
        report.push_str(&self.dependency_package);
        report.push('\n');
        report.push_str("current lock: ");
        report.push_str(&self.current_lock_fingerprint);
        report.push('\n');
        report.push_str("candidate lock: ");
        report.push_str(&self.candidate_lock.fingerprint());
        report.push('\n');
        if self.added_packages.is_empty() {
            report.push_str("added packages: none\n");
        } else {
            report.push_str("added packages: ");
            report.push_str(&self.added_packages.join(", "));
            report.push('\n');
        }
        report.push_str(&self.candidate_audit.to_text());
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageInstallPlanError {
    CurrentGraph(PackageGraphAuditError),
    MissingCurrentRootManifest {
        package: String,
    },
    AliasAlreadyBound {
        alias: String,
        package: String,
    },
    MissingCandidateRootManifest {
        package: String,
    },
    MissingCandidatePackageManifest {
        package: String,
    },
    MissingCandidateDependencyAlias {
        root_package: String,
        alias: String,
        expected_package: String,
    },
    CandidateDependencyAliasMismatch {
        root_package: String,
        alias: String,
        expected_package: String,
        actual_package: String,
    },
    CandidateLock(PackageLockAssemblyError),
    CandidateGraph(PackageGraphAuditError),
}

pub fn plan_package_install(
    current_lock: &PackageLock,
    current_manifests: &[PackageCapabilityManifest],
    candidate_manifests: &[PackageCapabilityManifest],
    dependency_alias: &AliasName,
    dependency_package: &PackageName,
) -> Result<PackageInstallPlan, PackageInstallPlanError> {
    audit_package_graph(current_lock, current_manifests)
        .map_err(PackageInstallPlanError::CurrentGraph)?;

    let root_package = current_lock.root_package.clone();
    let current_root = manifest_for(current_manifests, &root_package).ok_or_else(|| {
        PackageInstallPlanError::MissingCurrentRootManifest {
            package: root_package.as_str().to_owned(),
        }
    })?;
    if let Some(existing) = current_root
        .normalized_clone()
        .dependency_aliases
        .into_iter()
        .find(|dependency| &dependency.alias == dependency_alias)
    {
        return Err(PackageInstallPlanError::AliasAlreadyBound {
            alias: dependency_alias.as_str().to_owned(),
            package: existing.package.as_str().to_owned(),
        });
    }

    let candidate_root = manifest_for(candidate_manifests, &root_package).ok_or_else(|| {
        PackageInstallPlanError::MissingCandidateRootManifest {
            package: root_package.as_str().to_owned(),
        }
    })?;
    let candidate_dependency =
        manifest_for(candidate_manifests, dependency_package).ok_or_else(|| {
            PackageInstallPlanError::MissingCandidatePackageManifest {
                package: dependency_package.as_str().to_owned(),
            }
        })?;
    let candidate_root = candidate_root.normalized_clone();
    let Some(binding) = candidate_root
        .dependency_aliases
        .iter()
        .find(|dependency| &dependency.alias == dependency_alias)
    else {
        return Err(PackageInstallPlanError::MissingCandidateDependencyAlias {
            root_package: root_package.as_str().to_owned(),
            alias: dependency_alias.as_str().to_owned(),
            expected_package: dependency_package.as_str().to_owned(),
        });
    };
    if binding.package != candidate_dependency.package {
        return Err(PackageInstallPlanError::CandidateDependencyAliasMismatch {
            root_package: root_package.as_str().to_owned(),
            alias: dependency_alias.as_str().to_owned(),
            expected_package: dependency_package.as_str().to_owned(),
            actual_package: binding.package.as_str().to_owned(),
        });
    }

    let candidate_lock = PackageLock::from_manifests(root_package, candidate_manifests)
        .map_err(PackageInstallPlanError::CandidateLock)?;
    let candidate_audit = audit_package_graph(&candidate_lock, candidate_manifests)
        .map_err(PackageInstallPlanError::CandidateGraph)?;
    let added_packages = added_packages(current_lock, &candidate_lock);

    Ok(PackageInstallPlan {
        dependency_alias: dependency_alias.as_str().to_owned(),
        dependency_package: dependency_package.as_str().to_owned(),
        current_lock_fingerprint: current_lock.fingerprint(),
        candidate_lock,
        candidate_audit,
        added_packages,
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

fn added_packages(current: &PackageLock, candidate: &PackageLock) -> Vec<String> {
    let current_packages = current
        .packages
        .iter()
        .map(|package| package.package.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    candidate
        .packages
        .iter()
        .filter_map(|package| {
            let package = package.package.as_str();
            if current_packages.contains(package) {
                None
            } else {
                Some(package.to_owned())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DependencyAlias, SourceIdentity};

    fn manifest(package: &str, resolved: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            package_name(package),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: resolved.to_owned(),
            },
        )
    }

    fn package_name(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn alias_name(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn dependency(alias: &str, package: &str) -> DependencyAlias {
        DependencyAlias {
            alias: alias_name(alias),
            package: package_name(package),
            source_fingerprint: format!("source:{package}"),
        }
    }

    #[test]
    fn install_plan_builds_candidate_lock_and_audit() {
        let current_root = manifest("graph-workbench", "commit:root");
        let current_lock =
            PackageLock::from_manifests(package_name("graph-workbench"), &[current_root.clone()])
                .expect("current lock");

        let mut candidate_root = current_root.clone();
        candidate_root
            .dependency_aliases
            .push(dependency("file_journal", "file-journal"));
        let mut child = manifest("file-journal", "commit:file");
        child
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let candidate_manifests = vec![candidate_root, child];

        let plan = plan_package_install(
            &current_lock,
            &[current_root],
            &candidate_manifests,
            &alias_name("file_journal"),
            &package_name("file-journal"),
        )
        .expect("install should plan");

        assert_eq!(plan.added_packages, vec!["file-journal".to_owned()]);
        assert!(
            plan.candidate_lock
                .package(&package_name("file-journal"))
                .is_some()
        );
        assert!(
            plan.candidate_audit
                .to_text()
                .contains("graph-workbench -> file-journal")
        );
        assert!(plan.to_text().contains("added packages: file-journal"));
    }

    #[test]
    fn install_plan_rejects_existing_root_alias() {
        let mut current_root = manifest("graph-workbench", "commit:root");
        current_root
            .dependency_aliases
            .push(dependency("math", "arithmetic-kernels"));
        let math = manifest("arithmetic-kernels", "commit:math");
        let current_manifests = vec![current_root.clone(), math.clone()];
        let current_lock =
            PackageLock::from_manifests(package_name("graph-workbench"), &current_manifests)
                .expect("current lock");

        let mut candidate_root = current_root.clone();
        candidate_root
            .dependency_aliases
            .push(dependency("file_journal", "file-journal"));
        let candidate_manifests = vec![
            candidate_root,
            math,
            manifest("file-journal", "commit:file"),
        ];

        assert_eq!(
            plan_package_install(
                &current_lock,
                &current_manifests,
                &candidate_manifests,
                &alias_name("math"),
                &package_name("file-journal"),
            ),
            Err(PackageInstallPlanError::AliasAlreadyBound {
                alias: "math".to_owned(),
                package: "arithmetic-kernels".to_owned(),
            })
        );
    }

    #[test]
    fn install_plan_rejects_candidate_without_requested_alias() {
        let current_root = manifest("graph-workbench", "commit:root");
        let current_lock =
            PackageLock::from_manifests(package_name("graph-workbench"), &[current_root.clone()])
                .expect("current lock");
        let candidate_manifests = vec![
            current_root.clone(),
            manifest("file-journal", "commit:file"),
        ];

        assert_eq!(
            plan_package_install(
                &current_lock,
                &[current_root],
                &candidate_manifests,
                &alias_name("file_journal"),
                &package_name("file-journal"),
            ),
            Err(PackageInstallPlanError::MissingCandidateDependencyAlias {
                root_package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
                expected_package: "file-journal".to_owned(),
            })
        );
    }
}
