//! Reconcile package bindings and mount their acquired source namespaces.
//!
//! Invalid roots, names, aliases, edges and cycles reject at construction.
//! Captured roots are mounted lexically without reopening the checkout;
//! physical adapters retain their own source-root validity checks.

use super::dependency_graph::dependency_cycle;
use super::{
    PackageCompilationInputError, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationSourceRecord, PackageCompilationTargetInputs, PackageDependencyBinding,
    PackageSourceBinding, display_identity,
};
use crate::BuildDeclarationKind;
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
            // Acquired bytes may belong to any package. Only the selected root
            // exposes their metadata through this invocation's Build::Source.
            // Explicit root metadata on a foreign binding still refuses above.
            let canonical_source_metadata = if package.identity == root {
                package.canonical_source_metadata().cloned()
            } else {
                None
            };
            let canonical_root = match if package.captured_source.is_some() {
                if package.source_root.is_absolute() {
                    Ok(package.source_root.clone())
                } else {
                    Err("captured source root must be absolute".to_owned())
                }
            } else {
                canonical_source_root(&package.source_root)
            } {
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
            if canonical_packages
                .insert(
                    package.identity,
                    PackageCompilationSourceRecord {
                        source_root: canonical_root.clone(),
                        canonical_name: package.canonical_name,
                        canonical_source_metadata,
                        captured_source: package.captured_source,
                        captured_build_source: package.captured_build_source,
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

        append_overlapping_source_roots(&roots, &canonical_packages, &mut errors);

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

        let mut filesystem = source::SourceFileSystem::default();
        for (identity, record) in &canonical_packages {
            if let Some(tree) = &record.captured_source
                && let Err(error) =
                    filesystem.add_capture(record.source_root.clone(), Arc::clone(tree))
            {
                errors.push(PackageCompilationInputError::InvalidSourceRoot {
                    identity: *identity,
                    path: record.source_root.clone(),
                    reason: error.to_string(),
                });
            }
        }

        if errors.is_empty() {
            Ok(Self {
                source: Arc::new(PackageCompilationSourceInputs {
                    filesystem,
                    root,
                    root_role,
                    packages: canonical_packages,
                    dependency_acquisitions: Vec::new(),
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
        let filesystem = self.source_filesystem();

        for (identity, record) in &self.source.packages {
            if record.captured_source.is_some() {
                continue;
            }
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
        match filesystem.canonicalize(root_path) {
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
}

pub(super) fn append_overlapping_source_roots(
    roots: &BTreeMap<PathBuf, PackageKeyIdentity>,
    packages: &BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
    errors: &mut Vec<PackageCompilationInputError>,
) {
    let mut overlaps = Vec::new();
    for (root, identity) in roots {
        // Canonical roots are absolute. Path ancestors preserve component and
        // platform-prefix semantics without confusing `a` with sibling `a-b`.
        // Only actual ancestor roots can overlap; disjoint pairs are not visited.
        for ancestor in root.ancestors().skip(1) {
            if let Some((ancestor_root, ancestor_identity)) = roots.get_key_value(ancestor) {
                // In-memory acquisition retains the authored routing roots. A
                // dependency may live inside its requester's directory, but
                // mounting it must not replace any captured observation. The
                // exact subtree comparison includes absent entries, link data
                // and executable flags. Physical or differing roots still
                // overlap ambiguously and reject below.
                if let (Some(parent), Some(child)) = (
                    packages[ancestor_identity].captured_source.as_ref(),
                    packages[identity].captured_source.as_ref(),
                ) {
                    let relative = root
                        .strip_prefix(ancestor_root)
                        .expect("ancestor is a component prefix");
                    let directory = parent.entries().any(|(path, node)| {
                        path == relative && matches!(node, source::CapturedSourceNode::Directory)
                    });
                    if directory
                        && parent
                            .entries()
                            .filter_map(|(path, node)| {
                                let member = path.strip_prefix(relative).ok()?;
                                (!member.as_os_str().is_empty()).then_some((member, node))
                            })
                            .eq(child.entries())
                    {
                        continue;
                    }
                }
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
