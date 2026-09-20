//! Constructs the package graph and validates inputs before compilation.
//! Physical Source capture and revalidation live in `source_snapshot`.

use crate::source_snapshot;
use crate::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind,
    PackageSourceConsumptionCommitment,
};
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use checked_interpreter::CanonicalFilesystemMetadataIndex;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use terminal_psi::TerminalPsiIdentity;

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
        self.canonical_source_metadata = Some(source_snapshot::capture(&canonical_root)?);
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

/// One requester-local alias selected by the reconciled package graph for one
/// authorized dependency scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyBinding {
    requester: PackageKeyIdentity,
    alias: String,
    target: PackageKeyIdentity,
    purpose: DependencyPurpose,
}

impl PackageDependencyBinding {
    /// One product-scope edge. Durable `PackageDependencyClosure` rows are
    /// product-only: build edges are compilation-local nameability and never
    /// join a persisted closure.
    pub fn new(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
    ) -> Self {
        Self::for_purpose(requester, alias, target, DependencyPurpose::Product)
    }

    pub fn for_purpose(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
        purpose: DependencyPurpose,
    ) -> Self {
        Self {
            requester,
            alias: alias.into(),
            target,
            purpose,
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

    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
    }

    /// The exact occurrence coordinate of this edge: requester, purpose,
    /// alias, and target together, so an immutable input assigned to one
    /// occurrence can never slide onto a different edge.
    pub fn occurrence(&self) -> BuildDependencyOccurrence {
        BuildDependencyOccurrence {
            requester: self.requester,
            purpose: self.purpose,
            alias: self.alias.clone(),
            target: self.target,
        }
    }
}

/// The exact dependency-occurrence coordinate an immutable build input is
/// assigned to: one reconciled edge identified by requester, purpose,
/// alias, and target. Two edges sharing a requester and target under
/// different aliases remain distinct occurrences, and a coordinate naming
/// no edge is an extra input the binding rejects rather than guesses at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildDependencyOccurrence {
    requester: PackageKeyIdentity,
    purpose: DependencyPurpose,
    alias: String,
    target: PackageKeyIdentity,
}

impl BuildDependencyOccurrence {
    /// The declared coordinate; membership against a reconciled graph is
    /// checked by `PackageCompilationInputs::has_dependency_occurrence`.
    pub fn new(
        requester: PackageKeyIdentity,
        purpose: DependencyPurpose,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
    ) -> Self {
        Self {
            requester,
            purpose,
            alias: alias.into(),
            target,
        }
    }

    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }

    pub const fn purpose(&self) -> DependencyPurpose {
        self.purpose
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
    root_role: BuildDeclarationKind,
    packages: Vec<PackageKeyIdentity>,
    dependencies: Vec<PackageDependencyBinding>,
}

/// Exact generated Omega source handed off by one successfully checked package
/// build. Construction remains compiler-private: carrying this value proves
/// only that one compiler run produced these bytes, not package admission.
/// Clones share the immutable bundle, including paths and dependency metadata;
/// each consumer still validates its own closure, custody, purpose, effective
/// target, and build execution profile. Equal product bytes do not establish that the
/// generator ran under the consumer's admitted build context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGeneratedSourceBundle {
    contents: Arc<PackageGeneratedSourceBundleContents>,
}

#[derive(Debug, PartialEq, Eq)]
struct PackageGeneratedSourceBundleContents {
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    dependency_closure: PackageDependencyClosure,
    source_consumption_commitment: PackageSourceConsumptionCommitment,
    sources: Vec<PackageGeneratedSource>,
}

impl PackageGeneratedSourceBundle {
    #[doc(hidden)]
    pub fn from_checked(
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
        target: target::TargetProfile,
        build_execution_profile: Option<target::TargetProfile>,
        dependency_closure: PackageDependencyClosure,
        source_consumption_commitment: PackageSourceConsumptionCommitment,
        sources: Vec<PackageGeneratedSource>,
    ) -> Self {
        Self {
            contents: Arc::new(PackageGeneratedSourceBundleContents {
                package,
                purpose,
                target,
                build_execution_profile,
                dependency_closure,
                source_consumption_commitment,
                sources,
            }),
        }
    }

    pub fn package(&self) -> PackageKeyIdentity {
        self.contents.package
    }

    /// The checked activation that produced the bundle, not the import edge's
    /// relative scope. Ordinary dependencies of a build helper are build
    /// instances too, even though the helper imports them through `depend`.
    pub fn purpose(&self) -> DependencyPurpose {
        self.contents.purpose
    }

    pub fn target(&self) -> target::TargetProfile {
        self.contents.target
    }

    /// The admitted profile that checked and executed the producer's build.
    /// None denotes the unprofiled host of this in-process compiler run, not
    /// permission to substitute a catalogued execution profile.
    pub fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.contents.build_execution_profile
    }

    pub fn dependency_closure(&self) -> &PackageDependencyClosure {
        &self.contents.dependency_closure
    }

    pub fn source_consumption_commitment(&self) -> PackageSourceConsumptionCommitment {
        self.contents.source_consumption_commitment
    }

    pub fn sources(&self) -> &[PackageGeneratedSource] {
        &self.contents.sources
    }
}

/// One dependency's canonical component description, attached to the exact
/// target compilation that selected that dependency's providers with
/// `CompositionMode::Independent`.
///
/// The bytes are the description the dependency's own compilation published;
/// `expected_subject` is the Terminal Psi identity that compilation observed
/// for the component module, not a field read out of the description. This
/// carrier is trusted for nothing: build settlement verifies the bytes under
/// the build's admission profile against the expected subject before any
/// selected plan may join the component, so a substituted, corrupt, or
/// partial description rejects there instead of being carried as evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndependentComponentDescription {
    package: PackageKeyIdentity,
    expected_subject: TerminalPsiIdentity,
    description: Arc<[u8]>,
}

impl IndependentComponentDescription {
    pub fn new(
        package: PackageKeyIdentity,
        expected_subject: TerminalPsiIdentity,
        description: Vec<u8>,
    ) -> Self {
        Self {
            package,
            expected_subject,
            description: Arc::from(description),
        }
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn expected_subject(&self) -> TerminalPsiIdentity {
        self.expected_subject
    }

    /// The exact canonical description bytes to verify. Their identity is
    /// established by the verifier, never by this carrier.
    pub fn description(&self) -> &[u8] {
        &self.description
    }
}

impl PackageDependencyClosure {
    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub const fn root_role(&self) -> BuildDeclarationKind {
        self.root_role
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
        root_role: BuildDeclarationKind,
        packages: Vec<PackageKeyIdentity>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, &'static str> {
        if root_role == BuildDeclarationKind::Workspace {
            return Err("package dependency closure root has workspace role");
        }
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
            if dependency.purpose != DependencyPurpose::Product {
                return Err("package dependency closure retains only product-scope edges");
            }
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

        if reachable_packages(root, |package| adjacency.get(&package)) != package_set {
            return Err("package dependency closure contains an unreachable package");
        }
        if dependency_cycle_in_set(&package_set, &adjacency) {
            return Err("package dependency closure contains a cycle");
        }

        Ok(Self {
            root,
            root_role,
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
    source: Arc<PackageCompilationSourceInputs>,
    target: PackageCompilationTargetInputs,
}

/// Checked activation purpose and exact-target attachments, independent of
/// the shared source graph.
/// These maps remain private and are revalidated when joined to another source
/// graph. An empty value represents no attached target-specific inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationTargetInputs {
    purpose: DependencyPurpose,
    dependency_generated_sources:
        BTreeMap<(PackageKeyIdentity, DependencyPurpose), PackageGeneratedSourceBundle>,
    accepted_semantic_bindings: BTreeMap<AcceptedSemanticBindingRole, AcceptedSemanticBinding>,
    independent_component_descriptions:
        BTreeMap<PackageKeyIdentity, IndependentComponentDescription>,
}

impl Default for PackageCompilationTargetInputs {
    fn default() -> Self {
        Self {
            purpose: DependencyPurpose::Product,
            dependency_generated_sources: BTreeMap::new(),
            accepted_semantic_bindings: BTreeMap::new(),
            independent_component_descriptions: BTreeMap::new(),
        }
    }
}

/// Shared target-independent package inputs used
/// while source roots and ordinary imports are discovered.
///
/// This is not a durable package identity: it deliberately retains physical
/// source roots and canonical build-visible metadata. Checkpoint equality
/// prevents a compiler source checkpoint prepared from one package graph from
/// being reused with a child whose source-routing inputs differ. Exact-target
/// generated sources and accepted semantic bindings are excluded because they
/// join only after that shared checkpoint forks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationSourceInputs {
    root: PackageKeyIdentity,
    root_role: BuildDeclarationKind,
    packages: BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
}

/// Source custody and requester-local routing owned by one package identity.
///
/// `dependencies` and `build_dependencies` are distinct scopes: the same alias
/// may bind different packages in each. Only the compilation root may hold
/// build-scope edges — the compiled program executes exactly one package's
/// build entry, and dependency build files never join it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageCompilationSourceRecord {
    source_root: PathBuf,
    canonical_name: String,
    canonical_source_metadata: Option<CanonicalFilesystemMetadataIndex>,
    dependencies: BTreeMap<String, PackageKeyIdentity>,
    build_dependencies: BTreeMap<String, PackageKeyIdentity>,
}

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

    pub fn root(&self) -> PackageKeyIdentity {
        self.source.root
    }

    pub fn root_role(&self) -> BuildDeclarationKind {
        self.source.root_role
    }

    pub fn package_root(&self, identity: PackageKeyIdentity) -> Option<&Path> {
        self.source
            .packages
            .get(&identity)
            .map(|record| record.source_root.as_path())
    }

    pub fn package_name(&self, identity: PackageKeyIdentity) -> Option<&str> {
        self.source
            .packages
            .get(&identity)
            .map(|record| record.canonical_name.as_str())
    }

    pub fn canonical_source_metadata(
        &self,
        identity: PackageKeyIdentity,
    ) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.source
            .packages
            .get(&identity)?
            .canonical_source_metadata
            .as_ref()
    }

    pub fn packages(&self) -> impl Iterator<Item = (PackageKeyIdentity, &Path)> {
        self.source
            .packages
            .iter()
            .map(|(identity, record)| (*identity, record.source_root.as_path()))
    }

    /// Share the exact target-independent source-routing inputs. Equality remains
    /// structural; pointer identity is not a source receipt or admission verdict.
    #[doc(hidden)]
    pub fn source_inputs(&self) -> Arc<PackageCompilationSourceInputs> {
        Arc::clone(&self.source)
    }

    /// Whether this reconciled graph contains the exact dependency
    /// occurrence — requester, purpose, alias, and target all naming one
    /// edge. Inputs keyed to a coordinate that is not an occurrence reject
    /// at binding rather than attaching to the nearest edge.
    pub fn has_dependency_occurrence(&self, occurrence: &BuildDependencyOccurrence) -> bool {
        let Some(record) = self.source.packages.get(&occurrence.requester) else {
            return false;
        };
        let edges = match occurrence.purpose {
            DependencyPurpose::Product => &record.dependencies,
            DependencyPurpose::Build => &record.build_dependencies,
        };
        edges.get(&occurrence.alias) == Some(&occurrence.target)
    }

    /// Separate shared source ownership from this invocation's target attachments.
    pub fn into_parts(
        self,
    ) -> (
        Arc<PackageCompilationSourceInputs>,
        PackageCompilationTargetInputs,
    ) {
        (self.source, self.target)
    }

    /// Rejoin target attachments only after checking their association to the
    /// supplied source graph. Nonempty generated inputs must cover every
    /// dependency; the unattached empty target state remains valid.
    pub fn from_parts(
        source: Arc<PackageCompilationSourceInputs>,
        target: PackageCompilationTargetInputs,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let PackageCompilationTargetInputs {
            purpose,
            dependency_generated_sources,
            accepted_semantic_bindings,
            independent_component_descriptions,
        } = target;
        let mut inputs = Self {
            source,
            target: PackageCompilationTargetInputs {
                purpose,
                ..PackageCompilationTargetInputs::default()
            },
        };
        inputs = inputs
            .with_accepted_semantic_bindings(accepted_semantic_bindings.into_values().collect())?;
        if !dependency_generated_sources.is_empty() {
            inputs = inputs.with_complete_dependency_generated_sources(
                dependency_generated_sources.into_values().collect(),
            )?;
        }
        inputs = inputs.with_independent_component_descriptions(
            independent_component_descriptions.into_values().collect(),
        )?;
        Ok(inputs)
    }

    /// Select this root's checked role without duplicating its acquired source
    /// graph. Changing roles never relabels retained outputs: reattachment must
    /// still supply the exact dependency instances the new role requires.
    pub fn with_compilation_purpose(
        self,
        purpose: DependencyPurpose,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let (source, mut target) = self.into_parts();
        target.purpose = purpose;
        Self::from_parts(source, target)
    }

    pub const fn compilation_purpose(&self) -> DependencyPurpose {
        self.target.purpose
    }

    fn instance_purpose(&self, import_scope: DependencyPurpose) -> DependencyPurpose {
        match import_scope {
            DependencyPurpose::Product => self.compilation_purpose(),
            DependencyPurpose::Build => DependencyPurpose::Build,
        }
    }

    /// Relative import scopes are distinct from checked activation purposes.
    /// Product edges inherit their importing activation; only a build edge
    /// switches to the execution-profile activation. Construct this finite
    /// roster from the validated graph, never from supplied output bundles.
    fn dependency_source_instances(&self) -> BTreeSet<(PackageKeyIdentity, DependencyPurpose)> {
        let mut instances = BTreeSet::new();
        let mut pending = vec![(self.source.root, DependencyPurpose::Product)];
        pending.extend(
            self.build_dependencies()
                .map(|(_, _, package)| (package, DependencyPurpose::Build)),
        );
        while let Some((package, scope)) = pending.pop() {
            if !instances.insert((package, scope)) {
                continue;
            }
            if let Some(record) = self.source.packages.get(&package) {
                pending.extend(
                    record
                        .dependencies
                        .values()
                        .map(|dependency| (*dependency, scope)),
                );
            }
        }
        instances.remove(&(self.source.root, DependencyPurpose::Product));
        instances
    }

    /// Checked dependency occurrences required by this compilation. Product
    /// imports inherit the root's purpose; build imports always select Build.
    /// Producers use the same roster that validates the complete handoff.
    pub fn required_dependency_source_instances(
        &self,
    ) -> BTreeSet<(PackageKeyIdentity, DependencyPurpose)> {
        self.dependency_source_instances()
            .into_iter()
            .map(|(package, scope)| (package, self.instance_purpose(scope)))
            .collect()
    }

    /// Product-scope edges only. Build-purpose edges are compilation-local
    /// nameability for the root's build entry and never join the durable
    /// closure projections produced from this iterator.
    pub fn dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// The root's build-scope edges. Only the root package may hold them.
    pub fn build_dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.source.packages.iter().flat_map(|(requester, record)| {
            record
                .build_dependencies
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// Attach the complete consumer-policy semantic bindings admitted for this
    /// compile request. These rows neither enter nor widen the dependency
    /// graph; every bound package must already be in the exact closure.
    pub fn with_accepted_semantic_bindings(
        mut self,
        bindings: Vec<AcceptedSemanticBinding>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let mut accepted = BTreeMap::new();
        for binding in bindings {
            let role = binding.role();
            let package = binding.package();
            if !self.source.packages.contains_key(&package) {
                errors.push(
                    PackageCompilationInputError::ForeignSemanticBindingPackage { role, package },
                );
                continue;
            }
            if accepted.insert(role, binding).is_some() {
                errors.push(PackageCompilationInputError::DuplicateSemanticBindingRole { role });
            }
        }
        if errors.is_empty() {
            self.target.accepted_semantic_bindings = accepted;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub fn accepted_semantic_binding(
        &self,
        role: AcceptedSemanticBindingRole,
    ) -> Option<&AcceptedSemanticBinding> {
        self.target.accepted_semantic_bindings.get(&role)
    }

    #[doc(hidden)]
    pub fn accepted_semantic_bindings(&self) -> impl Iterator<Item = &AcceptedSemanticBinding> {
        self.target.accepted_semantic_bindings.values()
    }

    /// Project the exact validated graph without source paths, package display
    /// names, immutable source resolutions, or source bytes.
    ///
    /// The durable closure is the product scope: packages reachable from the
    /// root through product edges plus the product edges between them.
    /// Build-scope packages retain compilation-local nameability through
    /// [`Self::dependency_target_for_purpose`] but never join this projection —
    /// host build inputs are not product review subjects.
    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        self.dependency_closure_for(self.source.root)
    }

    /// Attach the complete set of fresh compiler-issued generated-source
    /// bundles for this root's dependencies. Empty bundles are retained so an
    /// omitted dependency build cannot be confused with a build that handed
    /// off no generated source. Coverage is per checked purpose, not package:
    /// identical target profiles or bytes never let one role satisfy another.
    pub fn with_complete_dependency_generated_sources(
        mut self,
        bundles: Vec<PackageGeneratedSourceBundle>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let required = self.required_dependency_source_instances();
        let mut generated = BTreeMap::new();
        for bundle in bundles {
            let package = bundle.package();
            let purpose = bundle.purpose();
            if package == self.source.root {
                errors.push(PackageCompilationInputError::RootGeneratedSourceBundle { package });
                continue;
            }
            if !self.source.packages.contains_key(&package) {
                errors.push(PackageCompilationInputError::ForeignGeneratedSourceBundle { package });
                continue;
            }
            if !required.contains(&(package, purpose)) {
                errors.push(
                    PackageCompilationInputError::GeneratedSourceBundlePurposeMismatch {
                        package,
                        purpose,
                    },
                );
                continue;
            }
            if bundle.dependency_closure() != &self.dependency_closure_for(package) {
                errors.push(
                    PackageCompilationInputError::GeneratedSourceBundleClosureMismatch { package },
                );
            }
            if generated.insert((package, purpose), bundle).is_some() {
                errors
                    .push(PackageCompilationInputError::DuplicateGeneratedSourceBundle { package });
            }
        }
        for (package, purpose) in required {
            if !generated.contains_key(&(package, purpose)) {
                errors.push(PackageCompilationInputError::MissingGeneratedSourceBundle { package });
            }
        }
        if errors.is_empty() {
            self.target.dependency_generated_sources = generated;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    #[doc(hidden)]
    pub fn dependency_generated_source_bundles(
        &self,
    ) -> impl Iterator<Item = &PackageGeneratedSourceBundle> {
        self.target.dependency_generated_sources.values()
    }

    /// Each retained handoff together with the relative source scope in which
    /// it is consumed. The same build instance can serve both relative scopes
    /// inside a build helper, but never serves a product activation.
    pub fn dependency_generated_source_instances(
        &self,
    ) -> impl Iterator<Item = (DependencyPurpose, &PackageGeneratedSourceBundle)> {
        self.dependency_source_instances()
            .into_iter()
            .filter_map(|(package, scope)| {
                self.target
                    .dependency_generated_sources
                    .get(&(package, self.instance_purpose(scope)))
                    .map(|bundle| (scope, bundle))
            })
    }

    /// Attach the component descriptions published for this root's
    /// independently composed dependencies. Each description names one
    /// dependency in the exact closure, never the root; the set need not
    /// cover every dependency because only an `Independent` selection
    /// consumes one, and settlement rejects a selection left without its
    /// description rather than treating the edge as fused.
    pub fn with_independent_component_descriptions(
        mut self,
        descriptions: Vec<IndependentComponentDescription>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let mut attached = BTreeMap::new();
        for description in descriptions {
            let package = description.package();
            if package == self.source.root {
                errors.push(
                    PackageCompilationInputError::RootIndependentComponentDescription { package },
                );
                continue;
            }
            if !self.source.packages.contains_key(&package) {
                errors.push(
                    PackageCompilationInputError::ForeignIndependentComponentDescription {
                        package,
                    },
                );
                continue;
            }
            if attached.insert(package, description).is_some() {
                errors.push(
                    PackageCompilationInputError::DuplicateIndependentComponentDescription {
                        package,
                    },
                );
            }
        }
        if errors.is_empty() {
            self.target.independent_component_descriptions = attached;
            Ok(self)
        } else {
            Err(errors)
        }
    }

    /// The attached component descriptions in package-identity order. They
    /// are unverified bytes until build settlement admits them.
    pub fn independent_component_descriptions(
        &self,
    ) -> impl Iterator<Item = &IndependentComponentDescription> {
        self.target.independent_component_descriptions.values()
    }

    #[doc(hidden)]
    pub fn validate_dependency_generated_source_target(
        &self,
        selected_target: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_generated_sources
            .values()
            .filter_map(|bundle| {
                let expected_target = match bundle.purpose() {
                    DependencyPurpose::Product => selected_target,
                    DependencyPurpose::Build => bundle.build_execution_profile(),
                };
                (Some(bundle.target()) != expected_target).then_some(
                    PackageCompilationInputError::GeneratedSourceBundleTargetMismatch {
                        package: bundle.package(),
                        bundle_target: bundle.target(),
                        selected_target: expected_target,
                    },
                )
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Generated bytes retain the build context that produced them even when
    /// their product target and source bytes equal another activation's output.
    pub fn validate_dependency_generated_source_execution_profile(
        &self,
        execution_profile: Option<target::TargetProfile>,
    ) -> Result<(), Vec<PackageCompilationInputError>> {
        let errors = self
            .target
            .dependency_generated_sources
            .values()
            .filter(|bundle| bundle.build_execution_profile() != execution_profile)
            .map(|bundle| {
                PackageCompilationInputError::GeneratedSourceBundleExecutionProfileMismatch {
                    package: bundle.package(),
                    bundle_profile: bundle.build_execution_profile(),
                    execution_profile,
                }
            })
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
        import_scope: DependencyPurpose,
        relative_candidates: &[PathBuf],
    ) -> Result<Option<PathBuf>, &'static str> {
        let Some(bundle) = self
            .target
            .dependency_generated_sources
            .get(&(package, self.instance_purpose(import_scope)))
        else {
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
                    self.package_root(package)
                        .expect("validated bundle package retains its source root"),
                    &relative,
                ));
            }
        }
        Ok(matched)
    }

    /// The product-scope closure of one package inside this validated graph:
    /// its product-reachable packages and the product edges between them.
    /// Generated-source bundle custody compares against exactly this shape.
    #[doc(hidden)]
    pub fn dependency_closure_for(&self, root: PackageKeyIdentity) -> PackageDependencyClosure {
        let reachable = reachable_packages(root, |package| {
            self.source
                .packages
                .get(&package)
                .map(|record| &record.dependencies)
        });
        PackageDependencyClosure {
            root,
            root_role: if root == self.source.root {
                self.source.root_role
            } else {
                BuildDeclarationKind::Package
            },
            packages: self
                .source
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

    /// The product-scope target of `requester`'s `alias`, if declared.
    #[doc(hidden)]
    pub fn dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.dependency_target_for_purpose(requester, DependencyPurpose::Product, alias)
    }

    /// The target of `requester`'s `alias` in exactly one dependency scope.
    /// Scopes never fall back to each other: a miss here does not license
    /// probing the other scope.
    #[doc(hidden)]
    pub fn dependency_target_for_purpose(
        &self,
        requester: PackageKeyIdentity,
        purpose: DependencyPurpose,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        let record = self.source.packages.get(&requester)?;
        match purpose {
            DependencyPurpose::Product => record.dependencies.get(alias),
            DependencyPurpose::Build => record.build_dependencies.get(alias),
        }
        .copied()
    }

    /// Whether `requester` directly declared `owner` as a dependency in either
    /// scope. Which source files may *name* which scope is decided earlier at
    /// import binding; this package-level check confirms only that a declared
    /// direct edge exists for the selected declaration's owner.
    #[doc(hidden)]
    pub fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        requester == owner
            || self.source.packages.get(&requester).is_some_and(|record| {
                record.dependencies.values().any(|target| *target == owner)
                    || record
                        .build_dependencies
                        .values()
                        .any(|target| *target == owner)
            })
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
        self.source.packages.iter().find_map(|(identity, record)| {
            source.starts_with(&record.source_root).then_some(*identity)
        })
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

impl build_time_evaluation::BuildTimeSelectionAuthority for PackageCompilationInputs {
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
    InvalidRootRole {
        role: BuildDeclarationKind,
    },
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
    NonRootBuildDependency {
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
    RootIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    ForeignIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    DuplicateIndependentComponentDescription {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleClosureMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundlePurposeMismatch {
        package: PackageKeyIdentity,
        purpose: DependencyPurpose,
    },
    GeneratedSourceBundleCustodyMismatch {
        package: PackageKeyIdentity,
    },
    GeneratedSourceBundleTargetMismatch {
        package: PackageKeyIdentity,
        bundle_target: target::TargetProfile,
        selected_target: Option<target::TargetProfile>,
    },
    GeneratedSourceBundleExecutionProfileMismatch {
        package: PackageKeyIdentity,
        bundle_profile: Option<target::TargetProfile>,
        execution_profile: Option<target::TargetProfile>,
    },
    ForeignSemanticBindingPackage {
        role: AcceptedSemanticBindingRole,
        package: PackageKeyIdentity,
    },
    DuplicateSemanticBindingRole {
        role: AcceptedSemanticBindingRole,
    },
}

impl fmt::Display for PackageCompilationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRootRole { role } => write!(
                formatter,
                "package compilation root has inadmissible declaration role {role:?}"
            ),
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
                "package identity {} binds dependency alias `{alias}` more than once in one scope",
                display_identity(*requester)
            ),
            Self::NonRootBuildDependency { requester, alias } => write!(
                formatter,
                "package identity {} declares build dependency `{alias}`; only the compilation root package may declare build dependencies",
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
            Self::RootIndependentComponentDescription { package } => write!(
                formatter,
                "root package {} cannot attach its own component description; only a dependency compiled as its own component publishes one",
                display_identity(*package)
            ),
            Self::ForeignIndependentComponentDescription { package } => write!(
                formatter,
                "component description names foreign package {}",
                display_identity(*package)
            ),
            Self::DuplicateIndependentComponentDescription { package } => write!(
                formatter,
                "package {} has more than one component description",
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
            Self::GeneratedSourceBundlePurposeMismatch { package, purpose } => write!(
                formatter,
                "generated-source bundle for package {} has an unrequested {} activation",
                display_identity(*package),
                purpose.name()
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
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<none>"),
            ),
            Self::GeneratedSourceBundleExecutionProfileMismatch {
                package,
                bundle_profile,
                execution_profile,
            } => write!(
                formatter,
                "generated-source bundle for package {} was produced under build execution profile `{}` but compilation selected `{}`",
                display_identity(*package),
                bundle_profile
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<unprofiled host>"),
                execution_profile
                    .map(target::TargetProfile::target_name)
                    .unwrap_or("<unprofiled host>"),
            ),
            Self::ForeignSemanticBindingPackage { role, package } => write!(
                formatter,
                "accepted semantic binding {role:?} names package {} outside the compilation closure",
                display_identity(*package),
            ),
            Self::DuplicateSemanticBindingRole { role } => write!(
                formatter,
                "accepted semantic binding role {role:?} appears more than once",
            ),
        }
    }
}

impl std::error::Error for PackageCompilationInputError {}

fn append_overlapping_source_roots(
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

pub(super) fn canonical_source_root(path: &Path) -> Result<PathBuf, String> {
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

fn reachable_packages<'inputs>(
    root: PackageKeyIdentity,
    dependencies: impl Fn(PackageKeyIdentity) -> Option<&'inputs BTreeMap<String, PackageKeyIdentity>>,
) -> BTreeSet<PackageKeyIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(targets) = dependencies(identity) {
            pending.extend(targets.values().copied());
        }
    }
    reachable
}

fn dependency_cycle(
    packages: &BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
) -> Option<Vec<PackageKeyIdentity>> {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    fn visit(
        identity: PackageKeyIdentity,
        packages: &BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
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
        if let Some(record) = packages.get(&identity) {
            for target in record
                .dependencies
                .values()
                .chain(record.build_dependencies.values())
                .copied()
            {
                if let Some(cycle) = visit(target, packages, states, stack) {
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
        if let Some(cycle) = visit(identity, packages, &mut states, &mut stack) {
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
mod tests;
