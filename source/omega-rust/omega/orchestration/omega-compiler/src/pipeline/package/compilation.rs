use super::package_source_consumption::PackageSourceConsumptionCommitment;
use omega_build_output::PackageGeneratedSource;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// One stable package identity, its canonical declared name, and the canonical
/// source root from which this compilation may load it. The name is validated
/// diagnostic metadata and the path is custody/routing data; neither replaces
/// the opaque identity in semantic comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceBinding {
    identity: PackageKeyIdentity,
    canonical_name: String,
    source_root: PathBuf,
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
        }
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
    pub(crate) fn from_checked(
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
        let mut roots = BTreeMap::<PathBuf, PackageKeyIdentity>::new();

        for package in packages {
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

            if canonical_packages
                .insert(package.identity, canonical_root.clone())
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicatePackageIdentity {
                    identity: package.identity,
                });
            }
            canonical_names.insert(package.identity, package.canonical_name);
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

    pub(crate) fn dependency_generated_source_bundles(
        &self,
    ) -> impl Iterator<Item = &PackageGeneratedSourceBundle> {
        self.dependency_generated_sources.values()
    }

    pub(crate) fn validate_dependency_generated_source_target(
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

    pub(crate) fn generated_source_import_path(
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

    pub(crate) fn is_generated_source_logical_path(&self, path: &Path) -> bool {
        self.generated_source_at_logical_path(path).is_some()
    }

    fn dependency_closure_for(&self, root: PackageKeyIdentity) -> PackageDependencyClosure {
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

    pub(crate) fn dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.dependencies
            .get(&requester)
            .and_then(|aliases| aliases.get(alias))
            .copied()
    }

    pub(crate) fn allows_declaration_selection(
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

    pub(crate) fn package_label(&self, identity: PackageKeyIdentity) -> String {
        match self.package_name(identity) {
            Some(name) => format!("`{name}` ({})", display_identity(identity)),
            None => display_identity(identity),
        }
    }

    pub(crate) fn package_for_source(&self, source: &Path) -> Option<PackageKeyIdentity> {
        self.packages
            .iter()
            .find_map(|(identity, root)| source.starts_with(root).then_some(*identity))
    }

    pub(crate) fn validate_for_compilation(
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
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-package-compilation-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create temporary package tree");
            Self(path)
        }

        fn package(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).expect("create package root");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn identity(marker: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
    }

    fn generated_bundle(
        inputs: &PackageCompilationInputs,
        package: PackageKeyIdentity,
        target: omega_target::TargetProfile,
        commitment_marker: u8,
        sources: Vec<PackageGeneratedSource>,
    ) -> PackageGeneratedSourceBundle {
        PackageGeneratedSourceBundle::from_checked(
            package,
            target,
            inputs.dependency_closure_for(package),
            PackageSourceConsumptionCommitment::for_test([commitment_marker; 32]),
            sources,
        )
    }

    fn generated_source(relative_path: &[u8], bytes: &[u8]) -> PackageGeneratedSource {
        let tree = omega_build_output::replayed_single_ordinary_file(relative_path, bytes)
            .expect("test source must form a canonical retained output tree");
        omega_build_output::select_included_sources(&tree, &[relative_path.to_vec()])
            .expect("test source must be explicitly included")
            .pop()
            .expect("one included test source must be retained")
    }

    fn three_package_generated_inputs(tree: &TempTree) -> PackageCompilationInputs {
        PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", tree.package("root")),
                PackageSourceBinding::new(identity(2), "middle", tree.package("middle")),
                PackageSourceBinding::new(identity(3), "leaf", tree.package("leaf")),
            ],
            vec![
                PackageDependencyBinding::new(identity(1), "middle", identity(2)),
                PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
            ],
        )
        .expect("generated-source test graph should close")
    }

    #[test]
    fn requester_local_aliases_may_name_different_targets() {
        let tree = TempTree::new();
        let packages = (1..=4)
            .map(|marker| {
                PackageSourceBinding::new(
                    identity(marker),
                    format!("package-{marker}"),
                    tree.package(&marker.to_string()),
                )
            })
            .collect();
        let inputs = PackageCompilationInputs::new(
            identity(1),
            packages,
            vec![
                PackageDependencyBinding::new(identity(1), "shared", identity(2)),
                PackageDependencyBinding::new(identity(2), "shared", identity(3)),
                PackageDependencyBinding::new(identity(3), "leaf", identity(4)),
            ],
        )
        .expect("requester-local aliases should reconcile");

        assert_eq!(
            inputs.dependency_target(identity(1), "shared"),
            Some(identity(2))
        );
        assert_eq!(
            inputs.dependency_target(identity(2), "shared"),
            Some(identity(3))
        );
        assert_eq!(inputs.package_name(identity(1)), Some("package-1"));
        assert!(inputs.allows_declaration_selection(identity(1), identity(1)));
        assert!(inputs.allows_declaration_selection(identity(1), identity(2)));
        assert!(!inputs.allows_declaration_selection(identity(1), identity(3)));
        assert!(inputs.allows_declaration_selection(identity(2), identity(3)));
        assert!(
            inputs
                .package_label(identity(1))
                .starts_with("`package-1` (")
        );
    }

    #[test]
    fn noncanonical_package_names_reject_at_compiler_handoff() {
        let tree = TempTree::new();
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![PackageSourceBinding::new(
                identity(1),
                "not_canonical",
                tree.package("root"),
            )],
            Vec::new(),
        )
        .expect_err("compiler inputs must independently reject noncanonical package names");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidPackageName { identity: found, name }
                if *found == identity(1) && name == "not_canonical"
        )));
    }

    #[test]
    fn duplicate_aliases_and_unreachable_rows_reject() {
        let tree = TempTree::new();
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", tree.package("root")),
                PackageSourceBinding::new(identity(2), "first", tree.package("first")),
                PackageSourceBinding::new(identity(3), "second", tree.package("second")),
            ],
            vec![
                PackageDependencyBinding::new(identity(1), "dep", identity(2)),
                PackageDependencyBinding::new(identity(1), "dep", identity(2)),
            ],
        )
        .expect_err("duplicate alias and unreachable package must reject");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::DuplicateAlias { alias, .. } if alias == "dep"
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::UnreachablePackage { identity: found }
                if *found == identity(3)
        )));
    }

    #[test]
    fn overlapping_roots_and_cycles_reject() {
        let tree = TempTree::new();
        let root = tree.package("root");
        let nested = root.join("nested");
        fs::create_dir(&nested).expect("create nested package");
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root),
                PackageSourceBinding::new(identity(2), "nested", nested),
            ],
            vec![
                PackageDependencyBinding::new(identity(1), "child", identity(2)),
                PackageDependencyBinding::new(identity(2), "parent", identity(1)),
            ],
        )
        .expect_err("overlap and cycle must reject");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::OverlappingSourceRoots { .. }
        )));
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, PackageCompilationInputError::DependencyCycle { .. }))
        );
    }

    #[test]
    fn canonical_path_free_closure_recovery_rejects_open_unreachable_and_cyclic_graphs() {
        let packages = vec![identity(1), identity(2)];
        let unreachable = PackageDependencyClosure::from_canonical_parts(
            identity(1),
            packages.clone(),
            Vec::new(),
        )
        .expect_err("unreachable path-free closure package must reject");
        assert!(unreachable.contains("unreachable"));

        let open = PackageDependencyClosure::from_canonical_parts(
            identity(1),
            packages.clone(),
            vec![PackageDependencyBinding::new(
                identity(1),
                "dependency",
                identity(3),
            )],
        )
        .expect_err("open path-free closure edge must reject");
        assert!(open.contains("open edge"));

        let cyclic = PackageDependencyClosure::from_canonical_parts(
            identity(1),
            packages,
            vec![
                PackageDependencyBinding::new(identity(1), "dependency", identity(2)),
                PackageDependencyBinding::new(identity(2), "root", identity(1)),
            ],
        )
        .expect_err("cyclic path-free closure must reject");
        assert!(cyclic.contains("cycle"));
    }

    #[test]
    fn complete_generated_source_bundles_bind_owner_closure_target_and_bytes() {
        let tree = TempTree::new();
        let inputs = three_package_generated_inputs(&tree);
        let generated = generated_source(
            b"generated_api.omg",
            b"pub machine generated_value() -> u64 { 17 }\n",
        );
        let generated_digest = generated.digest();
        let middle = generated_bundle(
            &inputs,
            identity(2),
            omega_target::TargetProfile::WindowsX64,
            12,
            vec![generated],
        );
        let leaf = generated_bundle(
            &inputs,
            identity(3),
            omega_target::TargetProfile::WindowsX64,
            13,
            Vec::new(),
        );

        let inputs = inputs
            .with_complete_dependency_generated_sources(vec![leaf, middle])
            .expect("one exact bundle per dependency should attach");
        inputs
            .validate_dependency_generated_source_target(Some(
                omega_target::TargetProfile::WindowsX64,
            ))
            .expect("matching generated-source targets should validate");
        let logical = inputs
            .generated_source_import_path(identity(2), &[PathBuf::from("generated_api.omg")])
            .expect("compiler-issued generated path should remain canonical")
            .expect("generated module should resolve from retained custody");
        assert_eq!(
            logical,
            inputs
                .package_root(identity(2))
                .unwrap()
                .join(".omega/generated/generated_api.omg")
        );
        let retained = inputs
            .generated_source_at_logical_path(&logical)
            .expect("logical generated path should recover retained bytes");
        assert_eq!(retained.relative_path(), b"generated_api.omg");
        assert_eq!(
            retained.bytes(),
            b"pub machine generated_value() -> u64 { 17 }\n"
        );
        assert_eq!(retained.digest(), generated_digest);
    }

    #[test]
    fn compiler_consumes_retained_dependency_generated_source_without_a_physical_file() {
        let tree = TempTree::new();
        let root = tree.package("root-generated-consumer");
        let dependency = tree.package("dependency-generated-producer");
        fs::write(
            root.join("build.omg"),
            r#"target windows_x64 { }
machine build(builder: &mut Build) {
    builder.application("root-generated-consumer");
    builder.depend_as("dependency", Source::Path { location: "../dependency-generated-producer" });
}
"#,
        )
        .expect("write generated consumer build declaration");
        fs::write(
            root.join("main.omg"),
            r#"use dependency::generated_api;
pub machine consume_generated_value() -> u64 {
    generated_value()
}
"#,
        )
        .expect("write generated consumer source");

        let inputs = PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root-generated-consumer", root.clone()),
                PackageSourceBinding::new(
                    identity(2),
                    "dependency-generated-producer",
                    dependency.clone(),
                ),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "dependency",
                identity(2),
            )],
        )
        .expect("generated consumer graph should close");
        let bundle = generated_bundle(
            &inputs,
            identity(2),
            omega_target::TargetProfile::WindowsX64,
            12,
            vec![generated_source(
                b"generated_api.omg",
                b"pub machine generated_value() -> u64 { 17 }\n",
            )],
        );
        let inputs = inputs
            .with_complete_dependency_generated_sources(vec![bundle])
            .expect("consumer should receive the complete dependency bundle");

        let checked = crate::pipeline::checked_entry::compile_to_checked_with_packages(
            &root.join("main.omg"),
            Some("windows_x64"),
            inputs,
        )
        .expect("retained generated dependency source should enter initial frontend loading");
        assert!(
            !dependency
                .join(".omega/generated/generated_api.omg")
                .exists(),
            "dependency-generated source must remain compiler custody, not a physical snapshot mutation"
        );
        checked
            .verify_current_source_consumption()
            .expect("generated bytes should verify from retained custody after compilation");
        assert!(checked.source_consumption_commitment().is_some());
    }

    #[test]
    fn generated_source_bundle_omission_duplicate_foreign_root_and_closure_substitution_reject() {
        let tree = TempTree::new();
        let inputs = three_package_generated_inputs(&tree);
        let middle = generated_bundle(
            &inputs,
            identity(2),
            omega_target::TargetProfile::WindowsX64,
            12,
            Vec::new(),
        );
        let leaf = generated_bundle(
            &inputs,
            identity(3),
            omega_target::TargetProfile::WindowsX64,
            13,
            Vec::new(),
        );

        let missing = inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![middle.clone()])
            .expect_err("omitted explicit empty leaf bundle must reject");
        assert!(missing.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::MissingGeneratedSourceBundle { package }
                if *package == identity(3)
        )));

        let duplicate = inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![
                middle.clone(),
                middle.clone(),
                leaf.clone(),
            ])
            .expect_err("duplicate package bundle must reject");
        assert!(duplicate.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::DuplicateGeneratedSourceBundle { package }
                if *package == identity(2)
        )));

        let foreign = PackageGeneratedSourceBundle::from_checked(
            identity(4),
            omega_target::TargetProfile::WindowsX64,
            inputs.dependency_closure_for(identity(3)),
            PackageSourceConsumptionCommitment::for_test([14; 32]),
            Vec::new(),
        );
        let foreign_errors = inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), foreign])
            .expect_err("foreign bundle must reject");
        assert!(foreign_errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::ForeignGeneratedSourceBundle { package }
                if *package == identity(4)
        )));

        let root = PackageGeneratedSourceBundle::from_checked(
            identity(1),
            omega_target::TargetProfile::WindowsX64,
            inputs.dependency_closure(),
            PackageSourceConsumptionCommitment::for_test([11; 32]),
            Vec::new(),
        );
        let root_errors = inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![middle.clone(), leaf.clone(), root])
            .expect_err("root self-injection must reject");
        assert!(root_errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::RootGeneratedSourceBundle { package }
                if *package == identity(1)
        )));

        let wrong_closure = PackageGeneratedSourceBundle::from_checked(
            identity(2),
            omega_target::TargetProfile::WindowsX64,
            inputs.dependency_closure_for(identity(3)),
            PackageSourceConsumptionCommitment::for_test([12; 32]),
            Vec::new(),
        );
        let closure_errors = inputs
            .with_complete_dependency_generated_sources(vec![wrong_closure, leaf])
            .expect_err("bundle from another producer closure must reject");
        assert!(closure_errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package }
                if *package == identity(2)
        )));
    }

    #[test]
    fn generated_source_bundle_target_substitution_rejects_before_loading() {
        let tree = TempTree::new();
        let inputs = three_package_generated_inputs(&tree);
        let middle = generated_bundle(
            &inputs,
            identity(2),
            omega_target::TargetProfile::WindowsX64,
            12,
            Vec::new(),
        );
        let leaf = generated_bundle(
            &inputs,
            identity(3),
            omega_target::TargetProfile::WindowsX64,
            13,
            Vec::new(),
        );
        let inputs = inputs
            .with_complete_dependency_generated_sources(vec![middle, leaf])
            .expect("complete generated-source bundles should attach");
        let errors = inputs
            .validate_dependency_generated_source_target(Some(
                omega_target::TargetProfile::LinuxX64,
            ))
            .expect_err("cross-target generated-source substitution must reject");
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|error| matches!(
            error,
            PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
                bundle_target: omega_target::TargetProfile::WindowsX64,
                selected_target: Some(omega_target::TargetProfile::LinuxX64),
                ..
            }
        )));
    }

    #[test]
    fn missing_and_symlink_source_roots_reject() {
        let tree = TempTree::new();
        let missing = tree.0.join("missing");
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", missing)],
            Vec::new(),
        )
        .expect_err("missing source root must reject");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let actual = tree.package("actual");
            let linked = tree.0.join("linked");
            symlink(actual, &linked).expect("create source-root symlink");
            let errors = PackageCompilationInputs::new(
                identity(1),
                vec![PackageSourceBinding::new(identity(1), "root", linked)],
                Vec::new(),
            )
            .expect_err("symlink source root must reject");
            assert!(errors.iter().any(|error| matches!(
                error,
                PackageCompilationInputError::InvalidSourceRoot { .. }
            )));
        }
    }
}
