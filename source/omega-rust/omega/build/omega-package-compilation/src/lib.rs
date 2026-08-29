#![forbid(unsafe_code)]

//! Reconciled package compilation inputs and exact source-consumption custody.

mod source_consumption;

use omega_build_output::PackageGeneratedSource;
use psi_checked_interpreter::{
    CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION, CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind, FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT,
};
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};
pub use source_consumption::{
    ConsumedSourceUnit, ConsumedSourceUnitKind, PackageCompilationSubject,
    PackageSourceConsumptionCommitment, derive_consumed_source_units,
    derive_package_compilation_subject, derive_source_consumption_commitment,
    toolchain_source_identities, toolchain_source_identity_digest, verify_current_files,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

const CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN: &[u8] = b"OMEGA-CANONICAL-BUILD-SOURCE-CONTENT-V1\0";
const CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT: u64 = 512 * 1024 * 1024;

/// One stable package identity, its canonical declared name, and the canonical
/// source root from which this compilation may load it. The name is validated
/// diagnostic metadata and the path is custody/routing data; neither replaces
/// the opaque identity in semantic comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceBinding {
    identity: PackageKeyIdentity,
    canonical_name: String,
    source_root: PathBuf,
    canonical_source_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

impl PackageSourceBinding {
    pub fn new(
        identity: PackageKeyIdentity,
        canonical_name: impl Into<String>,
        source_root: PathBuf,
    ) -> Self {
        Self {
            identity,
            canonical_name: canonical_name.into(),
            source_root,
            canonical_source_metadata: None,
        }
    }

    /// Capture compiler-owned canonical build-visible metadata from this exact
    /// physical root. Callers cannot supply rows or a content commitment.
    pub fn with_canonical_source_metadata(mut self) -> Result<Self, String> {
        let canonical_root = canonical_source_root(&self.source_root)?;
        self.canonical_source_metadata =
            Some(capture_canonical_source_metadata_root(&canonical_root)?);
        Ok(self)
    }

    pub const fn identity(&self) -> PackageKeyIdentity {
        self.identity
    }

    pub fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn canonical_source_metadata(&self) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_source_metadata.as_ref()
    }
}

/// One requester-local alias selected by the reconciled package graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyBinding {
    requester: PackageKeyIdentity,
    alias: String,
    target: PackageKeyIdentity,
}

impl PackageDependencyBinding {
    pub fn new(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
    ) -> Self {
        Self {
            requester,
            alias: alias.into(),
            target,
        }
    }

    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }

    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub const fn target(&self) -> PackageKeyIdentity {
        self.target
    }
}

/// Exact, source-path-free dependency closure consumed by one package-aware
/// compilation. This is a semantic subject coordinate, not source custody or
/// an admission verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyClosure {
    root: PackageKeyIdentity,
    packages: Vec<PackageKeyIdentity>,
    dependencies: Vec<PackageDependencyBinding>,
}

/// Exact generated Omega source handed off by one successfully checked package
/// build. Construction remains compiler-private: carrying this value proves
/// only that one compiler run produced these bytes, not package admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGeneratedSourceBundle {
    package: PackageKeyIdentity,
    target: omega_target::TargetProfile,
    dependency_closure: PackageDependencyClosure,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    sources: Vec<PackageGeneratedSource>,
}

impl PackageGeneratedSourceBundle {
    #[doc(hidden)]
    pub fn from_checked(
        package: PackageKeyIdentity,
        target: omega_target::TargetProfile,
        dependency_closure: PackageDependencyClosure,
        source_consumption_commitment: PackageSourceConsumptionCommitment,
        sources: Vec<PackageGeneratedSource>,
    ) -> Self {
        Self {
            package,
            target,
            dependency_closure,
            source_consumption_commitment,
            sources,
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> omega_target::TargetProfile {
        self.target
    }

    pub const fn dependency_closure(&self) -> &PackageDependencyClosure {
        &self.dependency_closure
    }

    pub const fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub fn sources(&self) -> &[PackageGeneratedSource] {
        &self.sources
    }
}

impl PackageDependencyClosure {
    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub fn packages(&self) -> &[PackageKeyIdentity] {
        &self.packages
    }

    pub fn dependencies(&self) -> &[PackageDependencyBinding] {
        &self.dependencies
    }

    /// Recover one canonical source-path-free closure from persisted semantic
    /// coordinates. This does not recover source custody or package admission.
    ///
    /// The wire decoder must not be able to manufacture a weaker graph shape
    /// than `PackageCompilationInputs`: packages and requester-local aliases
    /// are strictly ordered, every edge is closed, the root reaches every
    /// package, and cycles reject.
    #[doc(hidden)]
    pub fn from_canonical_parts(
        root: PackageKeyIdentity,
        packages: Vec<PackageKeyIdentity>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, &'static str> {
        if packages.is_empty() {
            return Err("package dependency closure has no packages");
        }
        if packages.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("package dependency closure packages are not in strict canonical order");
        }
        let package_set = packages.iter().copied().collect::<BTreeSet<_>>();
        if !package_set.contains(&root) {
            return Err("package dependency closure does not contain its root package");
        }

        let mut adjacency =
            BTreeMap::<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>::new();
        let mut prior_coordinate: Option<(PackageKeyIdentity, &str)> = None;
        for dependency in &dependencies {
            let coordinate = (dependency.requester, dependency.alias.as_str());
            if prior_coordinate.is_some_and(|prior| prior >= coordinate) {
                return Err("package dependency closure edges are not in strict canonical order");
            }
            prior_coordinate = Some(coordinate);
            if !is_snake_case(&dependency.alias) {
                return Err("package dependency closure contains a noncanonical alias");
            }
            if !package_set.contains(&dependency.requester)
                || !package_set.contains(&dependency.target)
            {
                return Err("package dependency closure contains an open edge");
            }
            adjacency
                .entry(dependency.requester)
                .or_default()
                .insert(dependency.alias.clone(), dependency.target);
        }

        if reachable_packages(root, &adjacency) != package_set {
            return Err("package dependency closure contains an unreachable package");
        }
        if dependency_cycle_in_set(&package_set, &adjacency) {
            return Err("package dependency closure contains a cycle");
        }

        Ok(Self {
            root,
            packages,
            dependencies,
        })
    }
}

/// Closed, requester-scoped package bindings accepted by package-aware
/// compilation. Construction validates the complete graph and canonicalizes
/// all source roots before the compiler can consume it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationInputs {
    root: PackageKeyIdentity,
    packages: BTreeMap<PackageKeyIdentity, PathBuf>,
    /// Canonical declared names retained solely for human-facing package
    /// diagnostics. Security decisions continue to compare exact identities.
    package_names: BTreeMap<PackageKeyIdentity, String>,
    canonical_source_metadata: BTreeMap<PackageKeyIdentity, CanonicalFilesystemMetadataIndex>,
    dependencies: BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
    dependency_generated_sources: BTreeMap<PackageKeyIdentity, PackageGeneratedSourceBundle>,
}

impl PackageCompilationInputs {
    pub fn new(
        root: PackageKeyIdentity,
        packages: Vec<PackageSourceBinding>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let mut canonical_packages = BTreeMap::new();
        let mut canonical_names = BTreeMap::new();
        let mut canonical_source_metadata = BTreeMap::new();
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
            if !is_kebab_case(&package.canonical_name) {
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
                && let Err(reason) =
                    validate_canonical_source_metadata_root(&canonical_root, metadata)
            {
                errors.push(PackageCompilationInputError::InvalidSourceRoot {
                    identity: package.identity,
                    path: canonical_root,
                    reason,
                });
                continue;
            }

            if canonical_packages
                .insert(package.identity, canonical_root.clone())
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicatePackageIdentity {
                    identity: package.identity,
                });
            }
            canonical_names.insert(package.identity, package.canonical_name);
            if let Some(metadata) = package.canonical_source_metadata {
                canonical_source_metadata.insert(package.identity, metadata);
            }
            if let Some(first) = roots.insert(canonical_root.clone(), package.identity) {
                errors.push(PackageCompilationInputError::DuplicateSourceRoot {
                    first,
                    duplicate: package.identity,
                    path: canonical_root,
                });
            }
        }

        let root_rows = roots.iter().collect::<Vec<_>>();
        for (index, (left_root, left_identity)) in root_rows.iter().enumerate() {
            for (right_root, right_identity) in root_rows.iter().skip(index + 1) {
                if left_root.starts_with(right_root) || right_root.starts_with(left_root) {
                    errors.push(PackageCompilationInputError::OverlappingSourceRoots {
                        first: **left_identity,
                        first_root: (*left_root).clone(),
                        second: **right_identity,
                        second_root: (*right_root).clone(),
                    });
                }
            }
        }

        if !canonical_packages.contains_key(&root) {
            errors.push(PackageCompilationInputError::MissingRootPackage { root });
        }

        let mut canonical_dependencies =
            BTreeMap::<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>::new();
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

            let aliases = canonical_dependencies
                .entry(dependency.requester)
                .or_default();
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
            let reachable = reachable_packages(root, &canonical_dependencies);
            for identity in canonical_packages.keys() {
                if !reachable.contains(identity) {
                    errors.push(PackageCompilationInputError::UnreachablePackage {
                        identity: *identity,
                    });
                }
            }
        }

        if let Some(cycle) = dependency_cycle(&canonical_packages, &canonical_dependencies) {
            errors.push(PackageCompilationInputError::DependencyCycle { cycle });
        }

        if errors.is_empty() {
            Ok(Self {
                root,
                packages: canonical_packages,
                package_names: canonical_names,
                canonical_source_metadata,
                dependencies: canonical_dependencies,
                dependency_generated_sources: BTreeMap::new(),
            })
        } else {
            Err(errors)
        }
    }

    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub fn package_root(&self, identity: PackageKeyIdentity) -> Option<&Path> {
        self.packages.get(&identity).map(PathBuf::as_path)
    }

    pub fn package_name(&self, identity: PackageKeyIdentity) -> Option<&str> {
        self.package_names.get(&identity).map(String::as_str)
    }

    pub fn canonical_source_metadata(
        &self,
        identity: PackageKeyIdentity,
    ) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_source_metadata.get(&identity)
    }

    pub fn packages(&self) -> impl Iterator<Item = (PackageKeyIdentity, &Path)> {
        self.packages
            .iter()
            .map(|(identity, root)| (*identity, root.as_path()))
    }

    pub fn dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.dependencies.iter().flat_map(|(requester, aliases)| {
            aliases
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// Project the exact validated graph without source paths, package display
    /// names, immutable source resolutions, or source bytes.
    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        PackageDependencyClosure {
            root: self.root,
            packages: self.packages.keys().copied().collect(),
            dependencies: self
                .dependencies()
                .map(|(requester, alias, target)| {
                    PackageDependencyBinding::new(requester, alias, target)
                })
                .collect(),
        }
    }

    /// Attach the complete set of fresh compiler-issued generated-source
    /// bundles for this root's dependencies. Empty bundles are retained so an
    /// omitted dependency build cannot be confused with a build that handed
    /// off no generated source.
    pub fn with_complete_dependency_generated_sources(
        mut self,
        bundles: Vec<PackageGeneratedSourceBundle>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let mut generated = BTreeMap::new();
        for bundle in bundles {
            let package = bundle.package();
            if package == self.root {
                errors.push(PackageCompilationInputError::RootGeneratedSourceBundle { package });
                continue;
            }
            if !self.packages.contains_key(&package) {
                errors.push(PackageCompilationInputError::ForeignGeneratedSourceBundle { package });
                continue;
            }
            if bundle.dependency_closure() != &self.dependency_closure_for(package) {
                errors.push(
                    PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package },
                );
            }
            if generated.insert(package, bundle).is_some() {
                errors
                    .push(PackageCompilationInputError::DuplicateGeneratedSourceBundle { package });
            }
        }
        for package in self
            .packages
            .keys()
            .copied()
            .filter(|package| *package != self.root)
        {
            if !generated.contains_key(&package) {
                errors.push(PackageCompilationInputError::MissingGeneratedSourceBundle { package });
            }
        }
        if errors.is_empty() {
            self.dependency_generated_sources = generated;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub fn dependency_generated_source_bundles(
        &self,
    ) -> impl Iterator<Item = &PackageGeneratedSourceBundle> {
        self.dependency_generated_sources.values()
    }

    #[doc(hidden)]
    pub fn validate_dependency_generated_source_target(
        &self,
        selected_target: Option<omega_target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .dependency_generated_sources
            .values()
            .filter(|bundle| Some(bundle.target()) != selected_target)
            .map(
                |bundle| PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
                    package: bundle.package(),
                    bundle_target: bundle.target(),
                    selected_target,
                },
            )
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub fn generated_source_import_path(
        &self,
        package: PackageKeyIdentity,
        relative_candidates: &[PathBuf],
    ) -> Result<Option<PathBuf>, &'static str> {
        let Some(bundle) = self.dependency_generated_sources.get(&package) else {
            return Ok(None);
        };
        let mut matched = None;
        for source in bundle.sources() {
            let relative = generated_source_relative_path(source)?;
            if relative_candidates
                .iter()
                .any(|candidate| candidate == &relative)
            {
                if matched.is_some() {
                    return Err("generated-source import resolves to more than one handoff");
                }
                matched = Some(generated_source_logical_path(
                    self.packages
                        .get(&package)
                        .expect("validated bundle package retains its source root"),
                    &relative,
                ));
            }
        }
        Ok(matched)
    }

    pub(crate) fn generated_source_at_logical_path(
        &self,
        path: &Path,
    ) -> Option<&PackageGeneratedSource> {
        self.dependency_generated_sources
            .iter()
            .find_map(|(package, bundle)| {
                let root = self.packages.get(package)?;
                bundle.sources().iter().find(|source| {
                    generated_source_relative_path(source).is_ok_and(|relative| {
                        generated_source_logical_path(root, &relative) == path
                    })
                })
            })
    }

    #[doc(hidden)]
    pub fn is_generated_source_logical_path(&self, path: &Path) -> bool {
        self.generated_source_at_logical_path(path).is_some()
    }

    #[doc(hidden)]
    pub fn dependency_closure_for(&self, root: PackageKeyIdentity) -> PackageDependencyClosure {
        let reachable = reachable_packages(root, &self.dependencies);
        PackageDependencyClosure {
            root,
            packages: self
                .packages
                .keys()
                .copied()
                .filter(|package| reachable.contains(package))
                .collect(),
            dependencies: self
                .dependencies()
                .filter(|(requester, _, target)| {
                    reachable.contains(requester) && reachable.contains(target)
                })
                .map(|(requester, alias, target)| {
                    PackageDependencyBinding::new(requester, alias, target)
                })
                .collect(),
        }
    }

    #[doc(hidden)]
    pub fn dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.dependencies
            .get(&requester)
            .and_then(|aliases| aliases.get(alias))
            .copied()
    }

    #[doc(hidden)]
    pub fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        requester == owner
            || self
                .dependencies
                .get(&requester)
                .is_some_and(|aliases| aliases.values().any(|target| *target == owner))
    }

    #[doc(hidden)]
    pub fn package_label(&self, identity: PackageKeyIdentity) -> String {
        match self.package_name(identity) {
            Some(name) => format!("`{name}` ({})", display_identity(identity)),
            None => display_identity(identity),
        }
    }

    #[doc(hidden)]
    pub fn package_for_source(&self, source: &Path) -> Option<PackageKeyIdentity> {
        self.packages
            .iter()
            .find_map(|(identity, root)| source.starts_with(root).then_some(*identity))
    }

    #[doc(hidden)]
    pub fn validate_for_compilation(
        &self,
        root_path: &Path,
        toolchain_root: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for (identity, expected_root) in &self.packages {
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
                    .packages
                    .get(&self.root)
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
        for (identity, root) in &self.packages {
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
        let mut diagnostics = Vec::new();
        for (identity, metadata) in &self.canonical_source_metadata {
            let root = self
                .packages
                .get(identity)
                .expect("canonical Source metadata retains a validated package root");
            if let Err(reason) = validate_canonical_source_metadata_root(root, metadata) {
                diagnostics.push(Diagnostic::error(format!(
                    "canonical Source metadata for package {} changed before compiler evidence was issued: {reason}",
                    display_identity(*identity)
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

fn validate_canonical_source_metadata_root(
    root: &Path,
    metadata: &CanonicalFilesystemMetadataIndex,
) -> Result<(), String> {
    let observed = capture_canonical_source_metadata_root(root)?;
    if &observed == metadata {
        Ok(())
    } else {
        Err(
            "canonical Source metadata or content commitment no longer matches the complete physical root"
                .to_owned(),
        )
    }
}

#[derive(Debug)]
struct CapturedPhysicalMetadataRow {
    kind: CanonicalFilesystemMetadataRowKind,
    content_digest: Option<[u8; 32]>,
}

fn capture_canonical_source_metadata_root(
    root: &Path,
) -> Result<CanonicalFilesystemMetadataIndex, String> {
    let mut stack = vec![(root.to_path_buf(), Vec::<u8>::new())];
    let mut rows = BTreeMap::<Vec<u8>, CapturedPhysicalMetadataRow>::new();
    let mut aggregate_path_bytes = 0usize;
    let mut aggregate_content_bytes = 0u64;

    while let Some((path, relative_path)) = stack.pop() {
        if rows.len() >= CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT {
            return Err(format!(
                "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
            ));
        }
        let physical = std::fs::symlink_metadata(&path).map_err(|error| {
            format!("could not inspect canonical Source metadata path: {error}")
        })?;
        let captured =
            capture_physical_metadata_row(&path, &physical, &mut aggregate_content_bytes)?;
        if rows.insert(relative_path.clone(), captured).is_some() {
            return Err(format!(
                "physical Source traversal duplicated a path: {relative_path:?}"
            ));
        }

        if physical.is_dir() {
            let children = std::fs::read_dir(&path).map_err(|error| {
                format!("could not enumerate canonical Source metadata directory: {error}")
            })?;
            for child in children {
                let child = child.map_err(|error| {
                    format!("could not enumerate canonical Source metadata entry: {error}")
                })?;
                if rows
                    .len()
                    .checked_add(stack.len())
                    .and_then(|count| count.checked_add(1))
                    .is_none_or(|count| count > CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT)
                {
                    return Err(format!(
                        "canonical Source metadata exceeds its {CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT}-row ceiling"
                    ));
                }
                let name = os_str_bytes(&child.file_name())?;
                let mut child_relative = relative_path.clone();
                if !child_relative.is_empty() {
                    child_relative.push(b'/');
                }
                child_relative.extend_from_slice(&name);
                aggregate_path_bytes = aggregate_path_bytes
                    .checked_add(child_relative.len())
                    .filter(|bytes| *bytes <= FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT)
                    .ok_or_else(|| {
                        format!(
                            "canonical Source metadata path bytes exceed {FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT}"
                        )
                    })?;
                if !psi_checked_interpreter::canonical_filesystem_metadata_path_is_canonical(
                    &child_relative,
                    false,
                ) {
                    return Err(format!(
                        "physical Source path is not canonical metadata: {child_relative:?}"
                    ));
                }
                stack.push((child.path(), child_relative));
            }
        }
    }

    let commitment = canonical_build_source_content_commitment(&rows)?;
    CanonicalFilesystemMetadataIndex::version_1(
        commitment,
        rows.into_iter()
            .map(|(path, row)| CanonicalFilesystemMetadataRow::new(path, row.kind)),
    )
    .map_err(|error| format!("could not construct canonical Source metadata: {error}"))
}

fn capture_physical_metadata_row(
    path: &Path,
    metadata: &std::fs::Metadata,
    aggregate_content_bytes: &mut u64,
) -> Result<CapturedPhysicalMetadataRow, String> {
    if metadata.is_dir() {
        #[cfg(unix)]
        require_canonical_mode(path, metadata, 0o555)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Directory,
            content_digest: None,
        });
    }
    if metadata.is_file() {
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o777;
            match mode {
                0o444 => false,
                0o555 => true,
                _ => {
                    return Err(format!(
                        "physical Source file {} has noncanonical mode {mode:#o}",
                        path.display()
                    ));
                }
            }
        };
        #[cfg(not(unix))]
        let executable = false;
        charge_canonical_source_content(aggregate_content_bytes, metadata.len())?;
        let content_digest = hash_canonical_source_file(path, metadata)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length: metadata.len(),
            },
            content_digest: Some(content_digest),
        });
    }
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(path).map_err(|error| {
            format!(
                "could not read canonical Source symlink {}: {error}",
                path.display()
            )
        })?;
        let target = os_str_bytes(target.as_os_str())?;
        let target_length = u64::try_from(target.len())
            .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
        charge_canonical_source_content(aggregate_content_bytes, target_length)?;
        return Ok(CapturedPhysicalMetadataRow {
            kind: CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length: target_length,
            },
            content_digest: Some(Sha256::digest(&target).into()),
        });
    }
    Err(format!(
        "physical Source path {} has an unsupported filesystem kind",
        path.display()
    ))
}

fn charge_canonical_source_content(total: &mut u64, amount: u64) -> Result<(), String> {
    *total = total
        .checked_add(amount)
        .filter(|total| *total <= CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT)
        .ok_or_else(|| {
            format!(
                "canonical Source content exceeds its {CANONICAL_BUILD_SOURCE_CONTENT_BYTE_LIMIT}-byte ceiling"
            )
        })?;
    Ok(())
}

fn hash_canonical_source_file(
    path: &Path,
    initial_metadata: &std::fs::Metadata,
) -> Result<[u8; 32], String> {
    let mut file = std::fs::File::open(path).map_err(|error| {
        format!(
            "could not open canonical Source file {}: {error}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut observed = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| {
            format!(
                "could not read canonical Source file {}: {error}",
                path.display()
            )
        })?;
        if count == 0 {
            break;
        }
        observed = observed
            .checked_add(u64::try_from(count).expect("fixed buffer read fits u64"))
            .filter(|observed| *observed <= initial_metadata.len())
            .ok_or_else(|| {
                format!(
                    "canonical Source file {} grew while captured",
                    path.display()
                )
            })?;
        digest.update(&buffer[..count]);
    }
    if observed != initial_metadata.len() {
        return Err(format!(
            "canonical Source file {} changed length while captured",
            path.display()
        ));
    }
    let final_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        format!(
            "could not recheck canonical Source file {}: {error}",
            path.display()
        )
    })?;
    if !final_metadata.is_file() || final_metadata.len() != initial_metadata.len() {
        return Err(format!(
            "canonical Source file {} changed identity while captured",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if final_metadata.dev() != initial_metadata.dev()
            || final_metadata.ino() != initial_metadata.ino()
            || final_metadata.mode() != initial_metadata.mode()
        {
            return Err(format!(
                "canonical Source file {} changed identity or mode while captured",
                path.display()
            ));
        }
    }
    Ok(digest.finalize().into())
}

fn canonical_build_source_content_commitment(
    rows: &BTreeMap<Vec<u8>, CapturedPhysicalMetadataRow>,
) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    digest.update(CANONICAL_BUILD_SOURCE_CONTENT_DOMAIN);
    digest.update(CANONICAL_FILESYSTEM_METADATA_POLICY_VERSION.to_le_bytes());
    digest.update(
        u64::try_from(rows.len())
            .map_err(|_| "canonical Source row count exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    for (path, row) in rows {
        hash_framed_bytes(&mut digest, path)?;
        match row.kind {
            CanonicalFilesystemMetadataRowKind::Directory => digest.update([0]),
            CanonicalFilesystemMetadataRowKind::File {
                executable,
                logical_byte_length,
            } => {
                digest.update([1, u8::from(executable)]);
                digest.update(logical_byte_length.to_le_bytes());
                digest.update(
                    row.content_digest
                        .ok_or_else(|| "canonical Source file omits content digest".to_owned())?,
                );
            }
            CanonicalFilesystemMetadataRowKind::Symlink {
                target_spelling_logical_byte_length,
            } => {
                digest.update([2]);
                digest.update(target_spelling_logical_byte_length.to_le_bytes());
                digest.update(
                    row.content_digest.ok_or_else(|| {
                        "canonical Source symlink omits content digest".to_owned()
                    })?,
                );
            }
        }
    }
    Ok(digest.finalize().into())
}

fn hash_framed_bytes(digest: &mut Sha256, bytes: &[u8]) -> Result<(), String> {
    digest.update(
        u64::try_from(bytes.len())
            .map_err(|_| "canonical Source path length exceeds u64".to_owned())?
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(())
}

#[cfg(unix)]
fn require_canonical_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected: u32,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o777;
    if mode == expected {
        Ok(())
    } else {
        Err(format!(
            "physical Source directory {} has noncanonical mode {mode:#o}; expected {expected:#o}",
            path.display()
        ))
    }
}

#[cfg(unix)]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(value.as_bytes().to_vec())
}

#[cfg(not(unix))]
fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    value
        .to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| "physical Source path is not portable UTF-8".to_owned())
}

impl psi_build_time_evaluation::BuildTimeSelectionAuthority for PackageCompilationInputs {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.allows_declaration_selection(requester, owner)
    }

    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        self.package_label(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCompilationInputError {
    InvalidPackageName {
        identity: PackageKeyIdentity,
        name: String,
    },
    InvalidSourceRoot {
        identity: PackageKeyIdentity,
        path: PathBuf,
        reason: String,
    },
    DuplicatePackageIdentity {
        identity: PackageKeyIdentity,
    },
    DuplicateSourceRoot {
        first: PackageKeyIdentity,
        duplicate: PackageKeyIdentity,
        path: PathBuf,
    },
    OverlappingSourceRoots {
        first: PackageKeyIdentity,
        first_root: PathBuf,
        second: PackageKeyIdentity,
        second_root: PathBuf,
    },
    MissingRootPackage {
        root: PackageKeyIdentity,
    },
    InvalidAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    MissingRequester {
        requester: PackageKeyIdentity,
    },
    MissingTarget {
        requester: PackageKeyIdentity,
        alias: String,
        target: PackageKeyIdentity,
    },
    DuplicateAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    UnreachablePackage {
        identity: PackageKeyIdentity,
    },
    DependencyCycle {
        cycle: Vec<PackageKeyIdentity>,
    },
    RootGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    ForeignGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    DuplicateGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    MissingGeneratedSourceBundle {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleClosureMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleCustodyMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleTargetMismatch {
        package: PackageKeyIdentity,
        bundle_target: omega_target::TargetProfile,
        selected_target: Option<omega_target::TargetProfile>,
    },
}

impl fmt::Display for PackageCompilationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPackageName { identity, name } => write!(
                formatter,
                "package identity {} has invalid canonical name `{name}`; expected lowercase kebab-case",
                display_identity(*identity)
            ),
            Self::InvalidSourceRoot {
                identity,
                path,
                reason,
            } => write!(
                formatter,
                "package identity {} has invalid source root {}: {reason}",
                display_identity(*identity),
                path.display()
            ),
            Self::DuplicatePackageIdentity { identity } => write!(
                formatter,
                "package identity {} has more than one source row",
                display_identity(*identity)
            ),
            Self::DuplicateSourceRoot {
                first,
                duplicate,
                path,
            } => write!(
                formatter,
                "package identities {} and {} share source root {}",
                display_identity(*first),
                display_identity(*duplicate),
                path.display()
            ),
            Self::OverlappingSourceRoots {
                first,
                first_root,
                second,
                second_root,
            } => write!(
                formatter,
                "package identities {} ({}) and {} ({}) have overlapping source roots",
                display_identity(*first),
                first_root.display(),
                display_identity(*second),
                second_root.display()
            ),
            Self::MissingRootPackage { root } => write!(
                formatter,
                "root package identity {} has no source row",
                display_identity(*root)
            ),
            Self::InvalidAlias { requester, alias } => write!(
                formatter,
                "package identity {} has invalid dependency alias `{alias}`",
                display_identity(*requester)
            ),
            Self::MissingRequester { requester } => write!(
                formatter,
                "dependency requester {} has no package source row",
                display_identity(*requester)
            ),
            Self::MissingTarget {
                requester,
                alias,
                target,
            } => write!(
                formatter,
                "dependency `{alias}` from {} targets missing package {}",
                display_identity(*requester),
                display_identity(*target)
            ),
            Self::DuplicateAlias { requester, alias } => write!(
                formatter,
                "package identity {} binds dependency alias `{alias}` more than once",
                display_identity(*requester)
            ),
            Self::UnreachablePackage { identity } => write!(
                formatter,
                "package identity {} is unreachable from the root package",
                display_identity(*identity)
            ),
            Self::DependencyCycle { cycle } => {
                write!(formatter, "package dependency cycle")?;
                for identity in cycle {
                    write!(formatter, " -> {}", display_identity(*identity))?;
                }
                Ok(())
            }
            Self::RootGeneratedSourceBundle { package } => write!(
                formatter,
                "root package {} cannot inject a generated-source bundle before its own build",
                display_identity(*package)
            ),
            Self::ForeignGeneratedSourceBundle { package } => write!(
                formatter,
                "generated-source bundle names foreign package {}",
                display_identity(*package)
            ),
            Self::DuplicateGeneratedSourceBundle { package } => write!(
                formatter,
                "package {} has more than one generated-source bundle",
                display_identity(*package)
            ),
            Self::MissingGeneratedSourceBundle { package } => write!(
                formatter,
                "dependency package {} has no generated-source bundle",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundleClosureMismatch { package } => write!(
                formatter,
                "generated-source bundle for package {} was produced from a different dependency closure",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundleCustodyMismatch { package } => write!(
                formatter,
                "generated-source bundle for package {} does not match its retained source custody and compiler review",
                display_identity(*package)
            ),
            Self::GeneratedSourceBundleTargetMismatch {
                package,
                bundle_target,
                selected_target,
            } => write!(
                formatter,
                "generated-source bundle for package {} targets `{}` but compilation selected `{}`",
                display_identity(*package),
                bundle_target.target_name(),
                selected_target
                    .map(omega_target::TargetProfile::target_name)
                    .unwrap_or("<none>"),
            ),
        }
    }
}

impl std::error::Error for PackageCompilationInputError {}

fn canonical_source_root(path: &Path) -> Result<PathBuf, String> {
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

fn generated_source_relative_path(
    source: &PackageGeneratedSource,
) -> Result<PathBuf, &'static str> {
    let mut path = PathBuf::new();
    for component in source.relative_path().split(|byte| *byte == b'/') {
        let component = std::str::from_utf8(component)
            .map_err(|_| "generated-source path is not canonical UTF-8")?;
        path.push(component);
    }
    Ok(path)
}

fn generated_source_logical_path(package_root: &Path, relative: &Path) -> PathBuf {
    package_root.join(".omega/generated").join(relative)
}

fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_kebab_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('-') {
        return false;
    }

    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
            continue;
        }
        previous_separator = false;
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

fn reachable_packages(
    root: PackageKeyIdentity,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> BTreeSet<PackageKeyIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(targets) = dependencies.get(&identity) {
            pending.extend(targets.values().copied());
        }
    }
    reachable
}

fn dependency_cycle(
    packages: &BTreeMap<PackageKeyIdentity, PathBuf>,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> Option<Vec<PackageKeyIdentity>> {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    fn visit(
        identity: PackageKeyIdentity,
        dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
        states: &mut BTreeMap<PackageKeyIdentity, Visit>,
        stack: &mut Vec<PackageKeyIdentity>,
    ) -> Option<Vec<PackageKeyIdentity>> {
        if states.get(&identity) == Some(&Visit::Complete) {
            return None;
        }
        if states.get(&identity) == Some(&Visit::Active) {
            let start = stack.iter().position(|entry| *entry == identity)?;
            let mut cycle = stack[start..].to_vec();
            cycle.push(identity);
            return Some(cycle);
        }

        states.insert(identity, Visit::Active);
        stack.push(identity);
        if let Some(targets) = dependencies.get(&identity) {
            for target in targets.values().copied() {
                if let Some(cycle) = visit(target, dependencies, states, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        states.insert(identity, Visit::Complete);
        None
    }

    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    for identity in packages.keys().copied() {
        if let Some(cycle) = visit(identity, dependencies, &mut states, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn dependency_cycle_in_set(
    packages: &BTreeSet<PackageKeyIdentity>,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> bool {
    let mut inbound = packages
        .iter()
        .copied()
        .map(|package| (package, 0usize))
        .collect::<BTreeMap<_, _>>();
    for targets in dependencies.values() {
        for target in targets.values() {
            let Some(count) = inbound.get_mut(target) else {
                return true;
            };
            let Some(next) = count.checked_add(1) else {
                return true;
            };
            *count = next;
        }
    }

    let mut ready = inbound
        .iter()
        .filter_map(|(package, count)| (*count == 0).then_some(*package))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(package) = ready.pop() {
        visited += 1;
        if let Some(targets) = dependencies.get(&package) {
            for target in targets.values() {
                let count = inbound
                    .get_mut(target)
                    .expect("closed package dependency edge retains its target");
                *count -= 1;
                if *count == 0 {
                    ready.push(*target);
                }
            }
        }
    }
    visited != packages.len()
}

fn display_identity(identity: PackageKeyIdentity) -> String {
    let digest = identity.digest();
    let mut display = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(display, "{byte:02x}");
    }
    display
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
