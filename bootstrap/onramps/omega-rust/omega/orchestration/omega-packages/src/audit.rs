use crate::lock::{PackageLock, PackageLockValidationError};
use crate::manifest::PackageCapabilityManifest;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraphAudit {
    pub root_package: String,
    pub packages: Vec<PackageGraphAuditPackage>,
    pub service_reach: Vec<PackageServiceReach>,
}

impl PackageGraphAudit {
    pub fn to_text(&self) -> String {
        let mut report = String::new();
        report.push_str("package graph audit\n");
        report.push_str("root: ");
        report.push_str(&self.root_package);
        report.push('\n');
        for package in &self.packages {
            report.push_str("- package: ");
            report.push_str(&package.package);
            report.push('\n');
            report.push_str("  path: ");
            report.push_str(&package.dependency_path.join(" -> "));
            report.push('\n');
            report.push_str("  source: ");
            report.push_str(&package.source_identity);
            report.push('\n');
            report.push_str("  manifest: ");
            report.push_str(&package.manifest_fingerprint);
            report.push('\n');
            report.push_str("  build observation: ");
            report.push_str(&package.build_observation);
            report.push('\n');
            if !package.exported_service_reach.is_empty() {
                report.push_str("  exported service reach: ");
                report.push_str(&package.exported_service_reach.join(", "));
                report.push('\n');
            }
        }
        if !self.service_reach.is_empty() {
            report.push_str("service reach paths\n");
            for service in &self.service_reach {
                report.push_str("- ");
                report.push_str(&service.service);
                report.push_str(" via ");
                report.push_str(&service.dependency_path.join(" -> "));
                report.push('\n');
            }
        }
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraphAuditPackage {
    pub package: String,
    pub source_identity: String,
    pub manifest_fingerprint: String,
    pub build_observation: String,
    pub dependency_path: Vec<String>,
    pub exported_service_reach: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageServiceReach {
    pub service: String,
    pub package: String,
    pub dependency_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageGraphAuditError {
    InvalidLock(Vec<PackageLockValidationError>),
    DuplicateManifest {
        package: String,
    },
    MissingManifest {
        package: String,
    },
    ManifestFingerprintMismatch {
        package: String,
        lock_fingerprint: String,
        manifest_fingerprint: String,
    },
    UnreachablePackage {
        package: String,
    },
}

pub fn audit_package_graph(
    lock: &PackageLock,
    manifests: &[PackageCapabilityManifest],
) -> Result<PackageGraphAudit, PackageGraphAuditError> {
    let lock = lock.normalized_clone();
    lock.validate_closure()
        .map_err(PackageGraphAuditError::InvalidLock)?;
    let manifests = manifest_map(manifests)?;
    let paths = dependency_paths(&lock);

    let mut packages = Vec::new();
    let mut service_reach = Vec::new();
    for locked in &lock.packages {
        let package_name = locked.package.as_str();
        let Some(path) = paths.get(package_name) else {
            return Err(PackageGraphAuditError::UnreachablePackage {
                package: package_name.to_owned(),
            });
        };
        let manifest =
            manifests
                .get(package_name)
                .ok_or_else(|| PackageGraphAuditError::MissingManifest {
                    package: package_name.to_owned(),
                })?;
        let manifest_fingerprint = manifest.fingerprint();
        if manifest_fingerprint != locked.manifest_fingerprint {
            return Err(PackageGraphAuditError::ManifestFingerprintMismatch {
                package: package_name.to_owned(),
                lock_fingerprint: locked.manifest_fingerprint.clone(),
                manifest_fingerprint,
            });
        }
        let exported_service_reach = manifest.normalized_clone().exported_service_reach;
        for service in &exported_service_reach {
            service_reach.push(PackageServiceReach {
                service: service.clone(),
                package: package_name.to_owned(),
                dependency_path: path.clone(),
            });
        }
        packages.push(PackageGraphAuditPackage {
            package: package_name.to_owned(),
            source_identity: locked.source_identity.clone(),
            manifest_fingerprint: locked.manifest_fingerprint.clone(),
            build_observation: locked.build_observation.clone(),
            dependency_path: path.clone(),
            exported_service_reach,
        });
    }

    packages.sort_by(|left, right| left.dependency_path.cmp(&right.dependency_path));
    service_reach.sort();
    Ok(PackageGraphAudit {
        root_package: lock.root_package.as_str().to_owned(),
        packages,
        service_reach,
    })
}

fn manifest_map(
    manifests: &[PackageCapabilityManifest],
) -> Result<BTreeMap<String, PackageCapabilityManifest>, PackageGraphAuditError> {
    let mut map = BTreeMap::new();
    for manifest in manifests {
        let package = manifest.package.as_str().to_owned();
        if map
            .insert(package.clone(), manifest.normalized_clone())
            .is_some()
        {
            return Err(PackageGraphAuditError::DuplicateManifest { package });
        }
    }
    Ok(map)
}

fn dependency_paths(lock: &PackageLock) -> BTreeMap<String, Vec<String>> {
    let packages = lock
        .packages
        .iter()
        .map(|package| (package.package.as_str().to_owned(), package))
        .collect::<BTreeMap<_, _>>();
    let root = lock.root_package.as_str().to_owned();
    let mut paths = BTreeMap::new();
    let mut queue = VecDeque::new();
    paths.insert(root.clone(), vec![root.clone()]);
    queue.push_back(root);

    while let Some(package_name) = queue.pop_front() {
        let Some(package) = packages.get(&package_name) else {
            continue;
        };
        let prefix = paths
            .get(&package_name)
            .expect("queued package should have path")
            .clone();
        for dependency in &package.dependencies {
            let dependency_name = dependency.package.as_str().to_owned();
            if paths.contains_key(&dependency_name) {
                continue;
            }
            let mut path = prefix.clone();
            path.push(dependency_name.clone());
            paths.insert(dependency_name.clone(), path);
            queue.push_back(dependency_name);
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::{LockedDependency, LockedPackage};
    use crate::manifest::{AliasName, PackageName, SourceIdentity};

    fn package(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn alias(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn manifest(package: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            PackageName::parse(package).unwrap(),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{package}"),
                resolved: format!("commit:{package}"),
            },
        )
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

    fn graph_lock(
        root_manifest: &PackageCapabilityManifest,
        child_manifest: &PackageCapabilityManifest,
    ) -> PackageLock {
        let mut root = locked_package(root_manifest);
        root.dependencies.push(LockedDependency {
            alias: alias("file_journal"),
            package: package("file-journal"),
        });
        let mut lock = PackageLock::new(root_manifest.package.clone());
        lock.packages = vec![root, locked_package(child_manifest)];
        lock
    }

    #[test]
    fn graph_audit_reports_dependency_path_for_service_reach() {
        let root_manifest = manifest("graph-workbench");
        let mut child_manifest = manifest("file-journal");
        child_manifest
            .exported_service_reach
            .push("FilesystemHost".to_owned());
        let lock = graph_lock(&root_manifest, &child_manifest);

        let audit = audit_package_graph(&lock, &[root_manifest, child_manifest])
            .expect("closed graph should audit");

        assert_eq!(audit.root_package, "graph-workbench");
        assert_eq!(
            audit.service_reach,
            vec![PackageServiceReach {
                service: "FilesystemHost".to_owned(),
                package: "file-journal".to_owned(),
                dependency_path: vec!["graph-workbench".to_owned(), "file-journal".to_owned()],
            }]
        );
        assert!(
            audit
                .to_text()
                .contains("FilesystemHost via graph-workbench -> file-journal")
        );
    }

    #[test]
    fn graph_audit_rejects_missing_manifest() {
        let root_manifest = manifest("graph-workbench");
        let child_manifest = manifest("file-journal");
        let lock = graph_lock(&root_manifest, &child_manifest);

        assert_eq!(
            audit_package_graph(&lock, &[root_manifest]),
            Err(PackageGraphAuditError::MissingManifest {
                package: "file-journal".to_owned()
            })
        );
    }

    #[test]
    fn graph_audit_rejects_manifest_fingerprint_drift() {
        let root_manifest = manifest("graph-workbench");
        let mut child_manifest = manifest("file-journal");
        let lock = graph_lock(&root_manifest, &child_manifest);
        child_manifest
            .exported_service_reach
            .push("FilesystemHost".to_owned());

        assert!(matches!(
            audit_package_graph(&lock, &[root_manifest, child_manifest]),
            Err(PackageGraphAuditError::ManifestFingerprintMismatch { package, .. })
                if package == "file-journal"
        ));
    }

    #[test]
    fn graph_audit_rejects_unreachable_lock_package() {
        let root_manifest = manifest("graph-workbench");
        let child_manifest = manifest("file-journal");
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![
            locked_package(&root_manifest),
            locked_package(&child_manifest),
        ];

        assert_eq!(
            audit_package_graph(&lock, &[root_manifest, child_manifest]),
            Err(PackageGraphAuditError::UnreachablePackage {
                package: "file-journal".to_owned()
            })
        );
    }
}
