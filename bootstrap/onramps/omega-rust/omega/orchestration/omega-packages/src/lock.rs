use crate::manifest::{AliasName, PackageName};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PACKAGE_LOCK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedDependency {
    pub alias: AliasName,
    pub package: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    pub package: PackageName,
    pub source_kind: String,
    pub source_locator: String,
    pub source_identity: String,
    pub manifest_fingerprint: String,
    pub build_observation: String,
    pub dependencies: Vec<LockedDependency>,
    pub trust_receipts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockValidationError {
    MissingRootPackage {
        package: String,
    },
    DuplicatePackage {
        package: String,
    },
    DuplicateDependencyAlias {
        package: String,
        alias: String,
    },
    MissingDependencyPackage {
        package: String,
        alias: String,
        dependency: String,
    },
    MissingSourceIdentity {
        package: String,
    },
    MissingManifestFingerprint {
        package: String,
    },
}

impl LockedPackage {
    pub fn normalized(mut self) -> Self {
        self.dependencies.sort();
        self.trust_receipts = sorted_unique(self.trust_receipts);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLock {
    pub schema_version: u32,
    pub root_package: PackageName,
    pub packages: Vec<LockedPackage>,
}

impl PackageLock {
    pub fn new(root_package: PackageName) -> Self {
        Self {
            schema_version: PACKAGE_LOCK_SCHEMA_VERSION,
            root_package,
            packages: Vec::new(),
        }
    }

    pub fn normalized(mut self) -> Self {
        self.packages = self
            .packages
            .into_iter()
            .map(LockedPackage::normalized)
            .collect();
        self.packages.sort();
        self
    }

    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(
            self.normalized_clone().to_json().as_bytes(),
        ))
    }

    pub fn normalized_clone(&self) -> Self {
        self.clone().normalized()
    }

    pub fn package(&self, package: &PackageName) -> Option<&LockedPackage> {
        self.packages.iter().find(|entry| &entry.package == package)
    }

    pub fn validate_closure(&self) -> Result<(), Vec<PackageLockValidationError>> {
        let mut errors = Vec::new();
        let mut package_names = BTreeSet::new();
        for package in &self.packages {
            if !package_names.insert(package.package.as_str().to_owned()) {
                errors.push(PackageLockValidationError::DuplicatePackage {
                    package: package.package.as_str().to_owned(),
                });
            }
            if package.source_identity.is_empty() {
                errors.push(PackageLockValidationError::MissingSourceIdentity {
                    package: package.package.as_str().to_owned(),
                });
            }
            if package.manifest_fingerprint.is_empty() {
                errors.push(PackageLockValidationError::MissingManifestFingerprint {
                    package: package.package.as_str().to_owned(),
                });
            }

            let mut aliases = BTreeSet::new();
            for dependency in &package.dependencies {
                if !aliases.insert(dependency.alias.as_str().to_owned()) {
                    errors.push(PackageLockValidationError::DuplicateDependencyAlias {
                        package: package.package.as_str().to_owned(),
                        alias: dependency.alias.as_str().to_owned(),
                    });
                }
            }
        }

        if !package_names.contains(self.root_package.as_str()) {
            errors.push(PackageLockValidationError::MissingRootPackage {
                package: self.root_package.as_str().to_owned(),
            });
        }
        for package in &self.packages {
            for dependency in &package.dependencies {
                if !package_names.contains(dependency.package.as_str()) {
                    errors.push(PackageLockValidationError::MissingDependencyPackage {
                        package: package.package.as_str().to_owned(),
                        alias: dependency.alias.as_str().to_owned(),
                        dependency: dependency.package.as_str().to_owned(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
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
        push_string_field(
            &mut json,
            1,
            "root_package",
            normalized.root_package.as_str(),
            true,
        );
        push_packages(&mut json, &normalized.packages);
        json.push_str("\n}\n");
        json
    }
}

fn push_packages(json: &mut String, packages: &[LockedPackage]) {
    push_indent(json, 1);
    push_json_string(json, "packages");
    json.push_str(": [");
    for (index, package) in packages.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        push_indent(json, 2);
        json.push_str("{\n");
        push_string_field(json, 3, "package", package.package.as_str(), true);
        push_string_field(json, 3, "source_kind", &package.source_kind, true);
        push_string_field(json, 3, "source_locator", &package.source_locator, true);
        push_string_field(json, 3, "source_identity", &package.source_identity, true);
        push_string_field(
            json,
            3,
            "manifest_fingerprint",
            &package.manifest_fingerprint,
            true,
        );
        push_string_field(
            json,
            3,
            "build_observation",
            &package.build_observation,
            true,
        );
        push_dependencies(json, &package.dependencies);
        push_string_array_field(json, 3, "trust_receipts", &package.trust_receipts, false);
        push_indent(json, 2);
        json.push('}');
    }
    if !packages.is_empty() {
        json.push('\n');
        push_indent(json, 1);
    }
    json.push(']');
}

fn push_dependencies(json: &mut String, dependencies: &[LockedDependency]) {
    push_indent(json, 3);
    push_json_string(json, "dependencies");
    json.push_str(": [");
    for (index, dependency) in dependencies.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        push_indent(json, 4);
        json.push('{');
        push_inline_string_field(json, "alias", dependency.alias.as_str(), true);
        push_inline_string_field(json, "package", dependency.package.as_str(), false);
        json.push('}');
    }
    if !dependencies.is_empty() {
        json.push('\n');
        push_indent(json, 3);
    }
    json.push_str("],\n");
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

fn push_string_array_field(
    json: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
    json.push(']');
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

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => json.push_str(&format!("\\u{:04x}", ch as u32)),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn alias(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn locked_package(name: &str) -> LockedPackage {
        LockedPackage {
            package: package(name),
            source_kind: "git".to_owned(),
            source_locator: format!("https://github.com/CathedralOS/{name}"),
            source_identity: "tree:abc".to_owned(),
            manifest_fingerprint: "manifest:def".to_owned(),
            build_observation: "pure".to_owned(),
            dependencies: Vec::new(),
            trust_receipts: Vec::new(),
        }
    }

    #[test]
    fn lock_json_and_fingerprint_are_normalized() {
        let mut left = PackageLock::new(package("graph-workbench"));
        let mut graph = locked_package("graph-workbench");
        graph.dependencies = vec![
            LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            },
            LockedDependency {
                alias: alias("arithmetic_kernels"),
                package: package("arithmetic-kernels"),
            },
        ];
        graph.trust_receipts = vec!["receipt:b".to_owned(), "receipt:a".to_owned()];
        left.packages = vec![graph.clone(), locked_package("file-journal")];

        let mut right = PackageLock::new(package("graph-workbench"));
        graph.dependencies.reverse();
        graph.trust_receipts.reverse();
        right.packages = vec![locked_package("file-journal"), graph];

        assert_eq!(left.to_json(), right.to_json());
        assert_eq!(left.fingerprint(), right.fingerprint());
        assert!(
            left.to_json()
                .contains("\"package\": \"arithmetic-kernels\"")
        );
    }

    #[test]
    fn lock_lookup_uses_canonical_package_name() {
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages.push(locked_package("file-journal"));

        assert!(lock.package(&package("file-journal")).is_some());
        assert!(PackageName::parse("file_journal").is_err());
    }

    #[test]
    fn lock_validation_accepts_closed_package_graph() {
        let mut root = locked_package("graph-workbench");
        root.dependencies.push(LockedDependency {
            alias: alias("file_journal"),
            package: package("file-journal"),
        });
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![root, locked_package("file-journal")];

        assert_eq!(lock.validate_closure(), Ok(()));
    }

    #[test]
    fn lock_validation_rejects_missing_root_package() {
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![locked_package("file-journal")];

        let errors = lock
            .validate_closure()
            .expect_err("missing root must reject");

        assert!(
            errors.contains(&PackageLockValidationError::MissingRootPackage {
                package: "graph-workbench".to_owned()
            })
        );
    }

    #[test]
    fn lock_validation_rejects_duplicate_packages() {
        let mut lock = PackageLock::new(package("file-journal"));
        lock.packages = vec![
            locked_package("file-journal"),
            locked_package("file-journal"),
        ];

        let errors = lock
            .validate_closure()
            .expect_err("duplicate package must reject");

        assert!(
            errors.contains(&PackageLockValidationError::DuplicatePackage {
                package: "file-journal".to_owned()
            })
        );
    }

    #[test]
    fn lock_validation_rejects_duplicate_alias_and_missing_dependency_target() {
        let mut root = locked_package("graph-workbench");
        root.dependencies = vec![
            LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            },
            LockedDependency {
                alias: alias("file_journal"),
                package: package("network-overreach"),
            },
        ];
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![root];

        let errors = lock
            .validate_closure()
            .expect_err("bad dependency closure must reject");

        assert!(
            errors.contains(&PackageLockValidationError::DuplicateDependencyAlias {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingDependencyPackage {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
                dependency: "file-journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingDependencyPackage {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
                dependency: "network-overreach".to_owned(),
            })
        );
    }

    #[test]
    fn lock_validation_rejects_missing_source_and_manifest_identity() {
        let mut locked = locked_package("file-journal");
        locked.source_identity.clear();
        locked.manifest_fingerprint.clear();
        let mut lock = PackageLock::new(package("file-journal"));
        lock.packages = vec![locked];

        let errors = lock
            .validate_closure()
            .expect_err("missing identities must reject");

        assert!(
            errors.contains(&PackageLockValidationError::MissingSourceIdentity {
                package: "file-journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingManifestFingerprint {
                package: "file-journal".to_owned(),
            })
        );
    }
}
