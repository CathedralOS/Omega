//! Builds the reconciled package graph from caller bindings, rejecting
//! every invalid root, name, source root, alias, edge, and cycle, and
//! revalidates the retained source roots before the compiler consumes them.

use super::dependency_graph::dependency_cycle;
use super::{
    PackageCompilationInputError, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationSourceRecord, PackageCompilationTargetInputs, PackageDependencyBinding,
    PackageSourceBinding, display_identity,
};
use crate::BuildDeclarationKind;
use crate::source_snapshot;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl PackageCompilationInputs {
    pub fn new(
        root: PackageKeyIdentity,
        root_role: BuildDeclarationKind,
        packages: Vec<PackageSourceBinding>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        if root_role == BuildDeclarationKind::Workspace {
            errors.push(PackageCompilationInputError::InvalidRootRole { role: root_role });
        }
        let mut canonical_packages = BTreeMap::new();
        let mut roots = BTreeMap::<PathBuf, PackageKeyIdentity>::new();

        for package in packages {
            if package.identity != root && package.canonical_source_metadata.is_some() {
                errors.push(PackageCompilationInputError::InvalidSourceRoot {
                    identity: package.identity,
                    path: package.source_root,
                    reason:
                        "only the current root package may retain canonical build Source metadata"
                            .to_owned(),
                });
                continue;
            }
            if !is_snake_case(&package.canonical_name) {
                errors.push(PackageCompilationInputError::InvalidPackageName {
                    identity: package.identity,
                    name: package.canonical_name.clone(),
                });
            }
            let canonical_root = match canonical_source_root(&package.source_root) {
                Ok(root) => root,
                Err(reason) => {
                    errors.push(PackageCompilationInputError::InvalidSourceRoot {
                        identity: package.identity,
                        path: package.source_root,
                        reason,
                    });
                    continue;
                }
            };
            if let Some(metadata) = &package.canonical_source_metadata
                && let Err(reason) = source_snapshot::validate_current(&canonical_root, metadata)
            {
                errors.push(PackageCompilationInputError::InvalidSourceRoot {
                    identity: package.identity,
                    path: canonical_root,
                    reason,
                });
                continue;
            }

            if canonical_packages
                .insert(
                    package.identity,
                    PackageCompilationSourceRecord {
                        source_root: canonical_root.clone(),
                        canonical_name: package.canonical_name,
                        canonical_source_metadata: package.canonical_source_metadata,
                        dependencies: BTreeMap::new(),
                        build_dependencies: BTreeMap::new(),
                    },
                )
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicatePackageIdentity {
                    identity: package.identity,
                });
            }
            if let Some(first) = roots.insert(canonical_root.clone(), package.identity) {
                errors.push(PackageCompilationInputError::DuplicateSourceRoot {
                    first,
                    duplicate: package.identity,
                    path: canonical_root,
                });
            }
        }

        append_overlapping_source_roots(&roots, &mut errors);

        if !canonical_packages.contains_key(&root) {
            errors.push(PackageCompilationInputError::MissingRootPackage { root });
        }

        for dependency in dependencies {
            if !is_snake_case(&dependency.alias) {
                errors.push(PackageCompilationInputError::InvalidAlias {
                    requester: dependency.requester,
                    alias: dependency.alias,
                });
                continue;
            }
            if !canonical_packages.contains_key(&dependency.requester) {
                errors.push(PackageCompilationInputError::MissingRequester {
                    requester: dependency.requester,
                });
                continue;
            }
            if !canonical_packages.contains_key(&dependency.target) {
                errors.push(PackageCompilationInputError::MissingTarget {
                    requester: dependency.requester,
                    alias: dependency.alias,
                    target: dependency.target,
                });
                continue;
            }

            let Some(requester) = canonical_packages.get_mut(&dependency.requester) else {
                continue;
            };
            let aliases = match dependency.purpose {
                DependencyPurpose::Product => &mut requester.dependencies,
                DependencyPurpose::Build => {
                    // Only this compilation's root executes a build entry:
                    // dependency build files never join the program, so a
                    // non-root build edge would name authority no source here
                    // could exercise. Reject it rather than retain a lie.
                    if dependency.requester != root {
                        errors.push(PackageCompilationInputError::NonRootBuildDependency {
                            requester: dependency.requester,
                            alias: dependency.alias,
                        });
                        continue;
                    }
                    &mut requester.build_dependencies
                }
            };
            if aliases
                .insert(dependency.alias.clone(), dependency.target)
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicateAlias {
                    requester: dependency.requester,
                    alias: dependency.alias,
                });
            }
        }

        if canonical_packages.contains_key(&root) {
            // The root's build entry may import build-scope packages, so its
            // build edges reach those snapshots and their product closures.
            // Every other package contributes product edges only: a
            // dependency's build context resolves in its own compilation,
            // where it is the root.
            let mut reachable = BTreeSet::new();
            let mut pending = vec![root];
            while let Some(identity) = pending.pop() {
                if !reachable.insert(identity) {
                    continue;
                }
                let Some(record) = canonical_packages.get(&identity) else {
                    continue;
                };
                pending.extend(record.dependencies.values().copied());
                if identity == root {
                    pending.extend(record.build_dependencies.values().copied());
                }
            }
            for identity in canonical_packages.keys() {
                if !reachable.contains(identity) {
                    errors.push(PackageCompilationInputError::UnreachablePackage {
                        identity: *identity,
                    });
                }
            }
        }

        if let Some(cycle) = dependency_cycle(&canonical_packages) {
            errors.push(PackageCompilationInputError::DependencyCycle { cycle });
        }

        if errors.is_empty() {
            Ok(Self {
                source: Arc::new(PackageCompilationSourceInputs {
                    root,
                    root_role,
                    packages: canonical_packages,
                }),
                target: PackageCompilationTargetInputs::default(),
            })
        } else {
            Err(errors)
        }
    }

    /// Construct an ordinary package compilation explicitly. Application
    /// roots must use [`Self::new`] with their retained authored role.
    pub fn new_package(
        root: PackageKeyIdentity,
        packages: Vec<PackageSourceBinding>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        Self::new(root, BuildDeclarationKind::Package, packages, dependencies)
    }

    #[doc(hidden)]
    pub fn validate_for_compilation(
        &self,
        root_path: &Path,
        toolchain_root: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for (identity, record) in &self.source.packages {
            let expected_root = &record.source_root;
            match canonical_source_root(expected_root) {
                Ok(actual_root) if actual_root == *expected_root => {}
                Ok(actual_root) => diagnostics.push(Diagnostic::error(format!(
                    "package source root {} changed identity before compilation (now {})",
                    expected_root.display(),
                    actual_root.display()
                ))),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "package source root {} is no longer valid for identity {}: {reason}",
                    expected_root.display(),
                    display_identity(*identity)
                ))),
            }
        }
        if let Err(mut metadata_diagnostics) = self.validate_canonical_source_metadata() {
            diagnostics.append(&mut metadata_diagnostics);
        }

        match root_path.canonicalize() {
            Ok(root_file) => {
                let expected_root = self
                    .package_root(self.source.root)
                    .expect("validated package graph retains its root");
                if !root_file.starts_with(expected_root) {
                    diagnostics.push(Diagnostic::error(format!(
                        "compilation root {} is outside reconciled root package {}",
                        root_file.display(),
                        expected_root.display()
                    )));
                }
            }
            Err(error) => diagnostics.push(Diagnostic::error(format!(
                "failed to canonicalize compilation root {}: {error}",
                root_path.display()
            ))),
        }

        let canonical_toolchain = toolchain_root
            .canonicalize()
            .unwrap_or_else(|_| toolchain_root.to_path_buf());
        for (identity, record) in &self.source.packages {
            let root = &record.source_root;
            if root.starts_with(&canonical_toolchain) || canonical_toolchain.starts_with(root) {
                diagnostics.push(Diagnostic::error(format!(
                    "package identity {} source root {} overlaps toolchain root {}",
                    display_identity(*identity),
                    root.display(),
                    canonical_toolchain.display()
                )));
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    #[doc(hidden)]
    pub fn validate_canonical_source_metadata(&self) -> Result<(), Vec<Diagnostic>> {
        // Construction permits build-visible metadata only on the current root.
        let Some(record) = self.source.packages.get(&self.source.root) else {
            return Ok(());
        };
        let Some(metadata) = &record.canonical_source_metadata else {
            return Ok(());
        };
        source_snapshot::validate_current(&record.source_root, metadata).map_err(|reason| {
            vec![Diagnostic::error(format!(
                "canonical Source metadata for package {} changed before compiler evidence was issued: {reason}",
                display_identity(self.source.root)
            ))]
        })
    }
}

pub(super) fn append_overlapping_source_roots(
    roots: &BTreeMap<PathBuf, PackageKeyIdentity>,
    errors: &mut Vec<PackageCompilationInputError>,
) {
    let mut overlaps = Vec::new();
    for (root, identity) in roots {
        // Canonical roots are absolute. Path ancestors preserve component and
        // platform-prefix semantics without confusing `a` with sibling `a-b`.
        // Only actual ancestor roots can overlap; disjoint pairs are not visited.
        for ancestor in root.ancestors().skip(1) {
            if let Some((ancestor_root, ancestor_identity)) = roots.get_key_value(ancestor) {
                overlaps.push((ancestor_root, root, ancestor_identity, identity));
            }
        }
    }
    // Keep the prior pairwise diagnostic order, independent of discovery order.
    overlaps.sort_unstable_by_key(|(first, second, _, _)| (*first, *second));
    errors.extend(
        overlaps
            .into_iter()
            .map(|(first_root, second_root, first, second)| {
                PackageCompilationInputError::OverlappingSourceRoots {
                    first: *first,
                    first_root: first_root.clone(),
                    second: *second,
                    second_root: second_root.clone(),
                }
            }),
    );
}

pub(crate) fn canonical_source_root(path: &Path) -> Result<PathBuf, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect source root: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("source root must not be a symbolic link".to_owned());
    }
    if !metadata.is_dir() {
        return Err("source root is not a directory".to_owned());
    }
    path.canonicalize()
        .map_err(|error| format!("cannot canonicalize source root: {error}"))
}

pub(super) fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
