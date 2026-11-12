/*
Reconciled package dependency graph, source-routing custody, and per-target
compilation attachments.

We construct and validate `PackageCompilationInputs`, the closed package input
contract consumed by package-aware compilation. We split compilation inputs
into two parts:
1. `PackageCompilationSourceInputs` (`Arc`-shared across target forks), which
   holds the root identity and role, canonical package roots, captured source
   trees, `Build::Source` metadata, and requester-scoped `Product` / `Build`
   dependency alias maps.
2. `PackageCompilationTargetInputs`, which holds the checked activation purpose,
   dependency `PackageBuildHandoff` bundles, accepted semantic bindings,
   independent component descriptions, and optional product retention coordinates.
*/

/*
Separating target-independent source routing from per-target attachments lets
us share one immutable Psi 00/01 parse checkpoint across target compilations
without ever reusing a checkpoint across divergent package graphs or permitting
target attachments to bypass closure and custody validation when rejoined via
`PackageCompilationInputs::from_parts`.
*/

/*
The caller-supplied binding carriers live in `bindings.rs`; `construction.rs`
validates and canonicalizes the package graph; `dependency_graph.rs` computes
reachable closures and cycle checks; `build_handoffs.rs`, `provider_selections.rs`,
`provider_defaults.rs`, and `native_bindings.rs` attach and admit dependency
Build outputs; and `errors.rs` defines `PackageCompilationInputError`.
*/

mod bindings;
mod build_handoffs;
mod construction;
mod dependency_graph;
mod errors;
mod native_bindings;
mod native_evaluation;
pub mod native_output_codec;
mod provider_defaults;
pub mod provider_selection_codec;
mod provider_selections;
pub use native_bindings::{
    NativeBindingProducer, NativeBindingProduction, PackageNativeBindingOrigin,
};
pub use native_evaluation::NativeBuildEvaluation;

pub use bindings::{
    IndependentComponentDescription, PackageDependencyBinding, PackageSourceBinding,
};
pub use build_handoffs::PackageBuildHandoff;
pub(crate) use construction::canonical_source_root;
pub use dependency_graph::PackageDependencyClosure;
pub use errors::PackageCompilationInputError;
pub use provider_defaults::{CompletedProviderDefaults, ProviderDefaultsAdmission};

use crate::{AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind};
use build_declarations::DependencyPurpose;
use filesystem_authority::CanonicalFilesystemMetadataIndex;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationInputs {
    source: Arc<PackageCompilationSourceInputs>,
    target: PackageCompilationTargetInputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationTargetInputs {
    purpose: DependencyPurpose,
    dependency_build_handoffs:
        BTreeMap<(PackageKeyIdentity, DependencyPurpose), PackageBuildHandoff>,
    accepted_semantic_bindings: BTreeMap<AcceptedSemanticBindingRole, AcceptedSemanticBinding>,
    semantic_role_discovery: bool,
    independent_component_descriptions:
        BTreeMap<PackageKeyIdentity, IndependentComponentDescription>,
    native_product_retention: Option<CompilerProductRetention>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerProductRetention {
    directory: PathBuf,
    input_key: [u8; 32],
    compiler_identity: [u8; 32],
}

impl CompilerProductRetention {
    pub fn new(directory: PathBuf, input_key: [u8; 32], compiler_identity: [u8; 32]) -> Self {
        Self {
            directory,
            input_key,
            compiler_identity,
        }
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub const fn input_key(&self) -> &[u8; 32] {
        &self.input_key
    }
    pub const fn compiler_identity(&self) -> &[u8; 32] {
        &self.compiler_identity
    }
}

impl Default for PackageCompilationTargetInputs {
    fn default() -> Self {
        Self {
            purpose: DependencyPurpose::Product,
            dependency_build_handoffs: BTreeMap::new(),
            accepted_semantic_bindings: BTreeMap::new(),
            semantic_role_discovery: false,
            independent_component_descriptions: BTreeMap::new(),
            native_product_retention: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct PackageCompilationSourceInputs {
    filesystem: source::SourceFileSystem,
    root: PackageKeyIdentity,
    root_role: BuildDeclarationKind,
    packages: BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
    dependency_acquisitions: Vec<PackageDependencyAcquisition>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct PackageDependencyAcquisition {
    pub request_ordinal: usize,
    pub request: build_declarations::BuildRequest,
    pub package: PackageKeyIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct PackageCompilationSourceRecord {
    source_root: PathBuf,
    canonical_name: String,
    canonical_source_metadata: Option<CanonicalFilesystemMetadataIndex>,
    captured_source: Option<Arc<source::CapturedSourceTree>>,
    captured_build_source: Option<build_output::CapturedBuildSourceInput>,
    dependencies: BTreeMap<String, PackageKeyIdentity>,
    build_dependencies: BTreeMap<String, PackageKeyIdentity>, // Only the compilation root may hold build-scope edges.
}

impl PackageCompilationInputs {
    pub fn with_native_product_retention(mut self, retention: CompilerProductRetention) -> Self {
        self.target.native_product_retention = Some(retention);
        self
    }

    pub fn native_product_retention(&self) -> Option<&CompilerProductRetention> {
        self.target.native_product_retention.as_ref()
    }

    pub(crate) fn require_same_shared_product_sources(&self, other: &Self) -> Result<(), String> {
        for (identity, record) in &self.source.packages {
            if let Some(previous) = other.source.packages.get(identity)
                && (record.source_root != previous.source_root
                    || record.canonical_name != previous.canonical_name
                    || record.captured_source != previous.captured_source
                    || record.captured_build_source != previous.captured_build_source
                    || record.dependencies != previous.dependencies)
            {
                return Err("Product query graphs disagree about a shared package source".into());
            }
            if other
                .source
                .packages
                .iter()
                .any(|(other_identity, other_record)| {
                    other_identity != identity && other_record.source_root == record.source_root
                })
            {
                return Err("Product query source root has different package owners".into());
            }
        }
        Ok(())
    }

    pub fn with_dependency_acquisitions(
        mut self,
        acquisitions: Vec<PackageDependencyAcquisition>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let invalid = |reason: &str| {
            vec![PackageCompilationInputError::InvalidDependencyAcquisition {
                root: self.root(),
                reason: reason.to_owned(),
            }]
        };
        let mut counts = [0usize; 2];
        for acquisition in &acquisitions {
            let build_declarations::BuildRequest::Dependency { purpose, alias, .. } =
                &acquisition.request
            else {
                return Err(invalid(
                    "acquisition does not describe a dependency request",
                ));
            };
            let position = if purpose.is_product() { 0 } else { 1 };
            if acquisition.request_ordinal != counts[position] {
                return Err(invalid(
                    "acquisitions do not retain purpose-local request order",
                ));
            }
            let selected_name = self
                .package_name(acquisition.package)
                .ok_or_else(|| invalid("selected dependency has no acquired source"))?;
            let alias = alias.as_ref().map_or(selected_name, |name| name.as_str());
            if self.dependency_target_for_purpose(self.root(), *purpose, alias)
                != Some(acquisition.package)
            {
                return Err(invalid(
                    "acquisition differs from its requester-local alias and purpose",
                ));
            }
            counts[position] += 1;
        }
        if counts[0] != self.direct_dependencies(DependencyPurpose::Product).count()
            || counts[1] != self.direct_dependencies(DependencyPurpose::Build).count()
        {
            return Err(invalid(
                "acquisitions do not cover the exact direct source graph",
            ));
        }
        Arc::make_mut(&mut self.source).dependency_acquisitions = acquisitions;
        Ok(self)
    }

    pub fn dependency_acquisitions(&self) -> &[PackageDependencyAcquisition] {
        &self.source.dependency_acquisitions
    }

    pub fn validate_execution_context(
        &self,
        selected_target: Option<target::TargetProfile>,
        build_profile: Option<target::TargetProfile>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        use diagnostics::Diagnostic;
        if self.compilation_purpose() == DependencyPurpose::Build
            && (build_profile.is_none() || selected_target != build_profile)
        {
            return Err(vec![Diagnostic::error(
                "build-purpose package compilation requires its target to equal the admitted build execution profile",
            )]);
        }
        let diagnostics = |errors: Vec<PackageCompilationInputError>| {
            errors
                .into_iter()
                .map(|error| Diagnostic::error(error.to_string()))
                .collect::<Vec<_>>()
        };
        self.validate_dependency_build_handoff_execution_profile(build_profile)
            .map_err(diagnostics)?;
        self.validate_dependency_build_handoff_target(selected_target)
            .map_err(diagnostics)
    }

    pub(crate) fn validate_unbound_source_join(
        &self,
        root: &Path,
        capture: &source::CapturedSourceTree,
    ) -> Result<(), String> {
        let record = self
            .source
            .packages
            .get(&self.source.root)
            .ok_or("joined package inputs lost their root")?;
        if record.source_root != root || record.captured_source.as_deref() != Some(capture) {
            return Err("joined package source differs from the original Build capture".into());
        }
        Ok(())
    }

    pub(crate) fn source_filesystem(&self) -> &source::SourceFileSystem {
        &self.source.filesystem
    }

    pub(crate) fn captured_source_input(
        &self,
        identity: PackageKeyIdentity,
    ) -> Option<&build_output::CapturedBuildSourceInput> {
        self.source
            .packages
            .get(&identity)?
            .captured_build_source
            .as_ref()
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

    #[doc(hidden)]
    pub fn source_inputs(&self) -> Arc<PackageCompilationSourceInputs> {
        Arc::clone(&self.source)
    }

    pub fn into_parts(
        self,
    ) -> (
        Arc<PackageCompilationSourceInputs>,
        PackageCompilationTargetInputs,
    ) {
        (self.source, self.target)
    }

    pub fn from_parts(
        source: Arc<PackageCompilationSourceInputs>,
        target: PackageCompilationTargetInputs,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let PackageCompilationTargetInputs {
            purpose,
            dependency_build_handoffs,
            accepted_semantic_bindings,
            semantic_role_discovery,
            independent_component_descriptions,
            native_product_retention,
        } = target;
        let mut inputs = Self {
            source,
            target: PackageCompilationTargetInputs {
                purpose,
                semantic_role_discovery,
                native_product_retention,
                ..PackageCompilationTargetInputs::default()
            },
        };
        inputs = inputs
            .with_accepted_semantic_bindings(accepted_semantic_bindings.into_values().collect())?;
        if !dependency_build_handoffs.is_empty() {
            inputs = inputs.with_complete_dependency_build_handoffs(
                dependency_build_handoffs.into_values().collect(),
            )?;
        }
        inputs = inputs.with_independent_component_descriptions(
            independent_component_descriptions.into_values().collect(),
        )?;
        Ok(inputs)
    }

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

    pub fn with_semantic_role_discovery(mut self, enabled: bool) -> Self {
        self.target.semantic_role_discovery = enabled;
        self
    }

    pub const fn semantic_role_discovery(&self) -> bool {
        self.target.semantic_role_discovery
    }

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

    pub fn independent_component_descriptions(
        &self,
    ) -> impl Iterator<Item = &IndependentComponentDescription> {
        self.target.independent_component_descriptions.values()
    }

    #[doc(hidden)]
    pub fn package_label(&self, identity: PackageKeyIdentity) -> String {
        match self.package_name(identity) {
            Some(name) => format!("`{name}` ({})", display_identity(identity)),
            None => display_identity(identity),
        }
    }

    /*
        Whether a source belongs to a validation subject of this compilation
        (wiki/spec/build/validation_subjects.md#subjects). The subject is the
        package the request names, which is this graph's root; a dependency
        from the graph is checked only through what the subject reaches, and
        keeps that answer when its own Build or product is compiled for the
        root. A source admitted without a package identity is the request's
        own authored source when the user, not the toolchain, supplied it.
    */
    pub fn is_validation_subject(inputs: Option<&Self>, file: &source::SourceFile) -> bool {
        match (inputs, file.package_identity) {
            (Some(inputs), Some(identity)) => identity == inputs.source.root,
            _ => file.origin == source::SourceOrigin::User,
        }
    }

    #[doc(hidden)]
    pub fn package_for_source(&self, source: &Path) -> Option<PackageKeyIdentity> {
        self.source
            .packages
            .iter()
            .filter(|(_, record)| source.starts_with(&record.source_root))
            .max_by_key(|(_, record)| record.source_root.components().count())
            .map(|(identity, _)| *identity)
    }
}

impl build_time_evaluation::BuildTimeSelectionAuthority for PackageCompilationInputs {
    fn declaration_selection_scope(
        &self,
    ) -> Option<BTreeMap<PackageKeyIdentity, Vec<PackageKeyIdentity>>> {
        Some(
            self.source
                .packages
                .iter()
                .map(|(requester, record)| {
                    let mut owners = vec![*requester];
                    owners.extend(record.dependencies.values().copied());
                    owners.extend(record.build_dependencies.values().copied());
                    owners.sort_unstable();
                    owners.dedup();
                    (*requester, owners)
                })
                .collect(),
        )
    }

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

fn display_identity(identity: PackageKeyIdentity) -> String {
    crate::digest_encoder::hex(&identity.digest())
}

#[cfg(test)]
mod acquisition_tests {
    use super::*;
    use build_declarations::{BuildDependencySource, BuildRequest, ProjectName};

    #[test]
    fn acquisition_order_and_aliases_are_scoped_by_purpose() {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "acquisition-inputs",
            false,
        )
        .unwrap();
        let root = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let child = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let root_path = temporary.path().join("root");
        let child_path = temporary.path().join("child");
        std::fs::create_dir(&root_path).unwrap();
        std::fs::create_dir(&child_path).unwrap();
        let purposes = [DependencyPurpose::Build, DependencyPurpose::Product];
        let inputs = PackageCompilationInputs::new(
            root,
            BuildDeclarationKind::Application,
            vec![
                PackageSourceBinding::new(root, "root", root_path),
                PackageSourceBinding::new(child, "child", child_path),
            ],
            purposes
                .iter()
                .map(|purpose| {
                    PackageDependencyBinding::for_purpose(root, "shared", child, *purpose)
                })
                .collect(),
        )
        .unwrap();
        let acquisitions = purposes
            .iter()
            .map(|purpose| PackageDependencyAcquisition {
                request_ordinal: 0,
                request: BuildRequest::Dependency {
                    purpose: *purpose,
                    alias: Some(ProjectName::parse("shared").unwrap()),
                    source: BuildDependencySource::Path("../child".into()),
                },
                package: child,
            })
            .collect::<Vec<_>>();
        let valid = inputs
            .clone()
            .with_dependency_acquisitions(acquisitions.clone())
            .unwrap();
        assert_eq!(valid.dependency_acquisitions(), acquisitions);
        let mut missing = acquisitions.clone();
        missing.pop();
        assert!(inputs.clone().with_dependency_acquisitions(missing).unwrap_err().iter().any(|error| matches!(error,
            PackageCompilationInputError::InvalidDependencyAcquisition { reason, .. } if reason.contains("exact direct source graph"))));
        let mut wrong_order = acquisitions.clone();
        wrong_order[1].request_ordinal = 1;
        assert!(inputs.clone().with_dependency_acquisitions(wrong_order).unwrap_err().iter().any(|error| matches!(error,
            PackageCompilationInputError::InvalidDependencyAcquisition { reason, .. } if reason.contains("purpose-local request order"))));
        let mut wrong_package = acquisitions;
        wrong_package[0].package = root;
        assert!(inputs.with_dependency_acquisitions(wrong_package).unwrap_err().iter().any(|error| matches!(error,
            PackageCompilationInputError::InvalidDependencyAcquisition { reason, .. } if reason.contains("alias and purpose"))));
    }
}
