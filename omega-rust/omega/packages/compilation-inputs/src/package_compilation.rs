//! Constructs the package graph and validates inputs before compilation.
//! Build metadata derives from retained source or sealed physical capture in
//! `source_snapshot`; source queries consume the invocation's shared view.
//!
//! The carriers a caller binds live in `bindings`; `construction` builds
//! and revalidates the reconciled graph; `dependency_graph` answers edge,
//! closure, and cycle questions over it; `generated_sources` attaches the
//! checked generated-source handoffs of dependency builds; `errors` names
//! every construction and attachment rejection.

mod bindings;
mod construction;
mod dependency_graph;
mod errors;
mod generated_sources;

pub use bindings::{
    IndependentComponentDescription, PackageDependencyBinding, PackageSourceBinding,
};
pub(crate) use construction::canonical_source_root;
pub use dependency_graph::PackageDependencyClosure;
pub use errors::PackageCompilationInputError;
pub use generated_sources::PackageGeneratedSourceBundle;

use crate::{AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind};
use build_declarations::DependencyPurpose;
use filesystem_authority::CanonicalFilesystemMetadataIndex;
use semantic_vocabulary::PackageKeyIdentity;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    semantic_role_discovery: bool,
    independent_component_descriptions:
        BTreeMap<PackageKeyIdentity, IndependentComponentDescription>,
}

impl Default for PackageCompilationTargetInputs {
    fn default() -> Self {
        Self {
            purpose: DependencyPurpose::Product,
            dependency_generated_sources: BTreeMap::new(),
            accepted_semantic_bindings: BTreeMap::new(),
            semantic_role_discovery: false,
            independent_component_descriptions: BTreeMap::new(),
        }
    }
}

/// Shared target-independent package inputs used
/// while source roots and ordinary imports are discovered.
///
/// This is not a durable package identity: it deliberately retains source
/// routing, immutable captured bytes and canonical build-visible metadata. Checkpoint equality
/// prevents a compiler source checkpoint prepared from one package graph from
/// being reused with a child whose source-routing inputs differ. Exact-target
/// generated sources and accepted semantic bindings are excluded because they
/// join only after that shared checkpoint forks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationSourceInputs {
    filesystem: source::SourceFileSystem,
    root: PackageKeyIdentity,
    root_role: BuildDeclarationKind,
    packages: BTreeMap<PackageKeyIdentity, PackageCompilationSourceRecord>,
    dependency_acquisitions: Vec<PackageDependencyAcquisition>,
}

/// A package-owned acquisition for an exact, purpose-local dependency request.
/// The request is inert data, not an issued execution receipt or a host grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyAcquisition {
    pub request_ordinal: usize,
    pub request: build_declarations::BuildRequest,
    pub package: PackageKeyIdentity,
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
    captured_source: Option<Arc<source::CapturedSourceTree>>,
    captured_build_source: Option<build_output::CapturedBuildSourceInput>,
    dependencies: BTreeMap<String, PackageKeyIdentity>,
    build_dependencies: BTreeMap<String, PackageKeyIdentity>,
}

impl PackageCompilationInputs {
    /// Retain source acquisition's exact requests and selected package custody.
    /// A later Build must issue these same values before the sources can settle
    /// its ledger. Merely supplying an alias graph is not that evidence.
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

    /// Admit the purpose and generated-source attachments of one compiler
    /// occurrence, whether supplied initially or acquired during its Build.
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
        self.validate_dependency_generated_source_execution_profile(build_profile)
            .map_err(diagnostics)?;
        self.validate_dependency_generated_source_target(selected_target)
            .map_err(diagnostics)
    }

    /// Retain the original root bytes when final ownership joins. Executed
    /// request settlement is checked by the Build continuation before assembly.
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

    /// Share the exact target-independent source-routing inputs. Equality remains
    /// structural; pointer identity is not a source receipt or admission verdict.
    #[doc(hidden)]
    pub fn source_inputs(&self) -> Arc<PackageCompilationSourceInputs> {
        Arc::clone(&self.source)
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
            semantic_role_discovery,
            independent_component_descriptions,
        } = target;
        let mut inputs = Self {
            source,
            target: PackageCompilationTargetInputs {
                purpose,
                semantic_role_discovery,
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

    /// Request semantic-role proposals during package review. A proposal fixes
    /// the contract being reviewed before its schema and provider commitments
    /// are derived; it grants no native binding or terminal authority. Ordinary
    /// compilation uses only the explicitly supplied accepted bindings.
    pub fn with_semantic_role_discovery(mut self, enabled: bool) -> Self {
        self.target.semantic_role_discovery = enabled;
        self
    }

    pub const fn semantic_role_discovery(&self) -> bool {
        self.target.semantic_role_discovery
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
    pub fn package_label(&self, identity: PackageKeyIdentity) -> String {
        match self.package_name(identity) {
            Some(name) => format!("`{name}` ({})", display_identity(identity)),
            None => display_identity(identity),
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
