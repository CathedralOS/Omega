//! The library a caller supplied for one Build before its first statement.

use super::ProductQueryDependency;
use crate::{AnonymousSourceCapture, PackageCompilationInputs};
use build_declarations::{
    BuildDeclarationKind, BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName,
};
use declaration_scope::PackageDeclarationScope;
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::compile_time_invocations::InvocationSourceConsent;
use source_compiler::representations::source_inputs::SourceMap;
use source_provenance::DependencyScope;
use source_text::SourceSpan;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuppliedBuildLibrary {
    request: BuildRequest,
    dependency: ProductQueryDependency,
}

impl SuppliedBuildLibrary {
    pub fn new(
        source: BuildDependencySource,
        dependency: ProductQueryDependency,
    ) -> Result<Self, String> {
        let inputs = &dependency.inputs;
        if inputs.root_role() != BuildDeclarationKind::Package
            || inputs.compilation_purpose() != DependencyPurpose::Build
        {
            return Err("a supplied Build library requires a package prepared for Build".into());
        }
        for package in inputs.dependency_closure().packages() {
            if inputs.captured_source_input(*package).is_none() {
                return Err(
                    "a supplied Build library requires its immutable Product sources".into(),
                );
            }
        }
        let handoff = dependency
            .generated
            .as_ref()
            .ok_or("a supplied Build library requires its checked package handoff")?;
        if handoff.package() != inputs.root()
            || handoff.purpose() != DependencyPurpose::Build
            || handoff.dependency_closure() != &inputs.dependency_closure()
        {
            return Err(
                "a supplied Build library has a handoff from another package or purpose".into(),
            );
        }
        let request = BuildRequest::Dependency {
            purpose: DependencyPurpose::Build,
            max_reach: dependency.max_reach.clone(),
            alias: Some(ProjectName::parse(&dependency.alias)?),
            source,
        };
        Ok(Self {
            request,
            dependency,
        })
    }

    fn handoff(&self) -> &crate::PackageBuildHandoff {
        self.dependency
            .generated
            .as_ref()
            .expect("the supplied library retains its checked handoff")
    }

    /// Defaults from this prepared library and the Products it acquired.
    pub fn provider_defaults(
        &self,
    ) -> Result<
        Vec<retained_product_selections::RetainedProviderSelection>,
        Vec<diagnostics::Diagnostic>,
    > {
        let inputs = &self.dependency.inputs;
        let mut defaults = inputs.admitted_provider_defaults(self.handoff())?;
        defaults.extend(inputs.admitted_package_provider_selections(DependencyPurpose::Product)?);
        Ok(defaults)
    }

    /// A library's own source keeps its Product edges. Only the receiving
    /// Build's check-only owner, when present, gets the caller's direct grants.
    pub fn declaration_scope(
        libraries: &[Self],
        check_owner: Option<PackageKeyIdentity>,
    ) -> Option<PackageDeclarationScope> {
        if libraries.is_empty() {
            return None;
        }
        let mut packages: std::collections::BTreeSet<_> = check_owner.into_iter().collect();
        let mut edges = Vec::new();
        for library in libraries {
            let closure = library.handoff().dependency_closure();
            packages.extend(closure.packages());
            edges.extend(
                closure
                    .dependencies()
                    .iter()
                    .map(|edge| (edge.requester(), edge.target())),
            );
            if let Some(requester) = check_owner {
                edges.push((requester, closure.root()));
            }
        }
        Some(PackageDeclarationScope::from_direct_dependencies(
            &packages, edges,
        ))
    }

    /// Select the handoff that supplied a declaration visible from this library's Product.
    pub fn provider_handoff(
        &self,
        owner: semantic_vocabulary::PackageKeyIdentity,
    ) -> Option<&crate::PackageBuildHandoff> {
        if self.dependency.inputs.root() == owner {
            return Some(self.handoff());
        }
        self.dependency
            .inputs
            .dependency_build_handoff_instances()
            .find(|(scope, handoff)| {
                *scope == DependencyPurpose::Product && handoff.package() == owner
            })
            .map(|(_, handoff)| handoff)
    }

    pub fn require_execution_profile(&self, profile: target::TargetProfile) -> Result<(), String> {
        let handoff = self.handoff();
        if handoff.target() != profile || handoff.build_execution_profile() != Some(profile) {
            return Err(
                "the supplied Build library was prepared for a different execution profile".into(),
            );
        }
        Ok(())
    }

    pub fn validate_handoff(&self, joined: &PackageCompilationInputs) -> Result<(), String> {
        let expected = self.handoff();
        if !joined
            .dependency_build_handoff_instances()
            .any(|(purpose, handoff)| purpose == DependencyPurpose::Build && handoff == expected)
        {
            return Err(
                "joined Product inputs differ from the handoff of a supplied Build library".into(),
            );
        }
        Ok(())
    }

    pub fn attenuated(
        &self,
        max_reach: &semantic_vocabulary::dependency_reach::DependencyReach,
    ) -> Result<Self, String> {
        if !max_reach.is_subset_of(self.max_reach()) {
            return Err(format!(
                "forwarded binding `{}` exceeds its incoming reach ceiling",
                self.dependency.alias
            ));
        }
        let mut forwarded = self.clone();
        if let BuildRequest::Dependency {
            max_reach: retained,
            ..
        } = &mut forwarded.request
        {
            *retained = max_reach.clone();
        }
        forwarded.dependency.max_reach = max_reach.clone();
        Ok(forwarded)
    }
    pub fn max_reach(&self) -> &semantic_vocabulary::dependency_reach::DependencyReach {
        let BuildRequest::Dependency { max_reach, .. } = &self.request else {
            unreachable!("a supplied library records a dependency")
        };
        max_reach
    }

    pub fn request(&self) -> &BuildRequest {
        &self.request
    }
    pub fn dependency(&self) -> &ProductQueryDependency {
        &self.dependency
    }

    /// Check the source occurrence behind this library, independently of the host
    /// profile selected for its code or any eventual Product deployment.
    pub fn matches_settled_source(&self, inputs: &PackageCompilationInputs) -> bool {
        let frozen = &self.dependency.inputs;
        frozen.root() == inputs.root()
            && frozen.dependency_closure() == inputs.dependency_closure()
            && frozen.require_same_shared_product_sources(inputs).is_ok()
    }

    /// An identical package key can select different captured bytes on two invocations.
    pub fn validate_join(&self, joined: &PackageCompilationInputs) -> Result<(), String> {
        self.validate_join_for(joined, DependencyPurpose::Build)
    }

    pub fn validate_join_for(
        &self,
        joined: &PackageCompilationInputs,
        purpose: DependencyPurpose,
    ) -> Result<(), String> {
        let library = &self.dependency.inputs;
        if joined.dependency_target_for_purpose(
            joined.root(),
            purpose,
            &self.dependency.alias,
        ) != Some(library.root())
            || joined.dependency_closure_for(library.root()) != library.dependency_closure()
        {
            return Err(
                "joined inputs differ from the supplied Build library or its Product dependencies"
                    .into(),
            );
        }
        library.require_same_shared_product_sources(joined)
    }
}

/// Authorize the caller's direct Build library selections only on the original
/// source map. The receiver continues to have invocation-local declarations.
struct SuppliedLibraryConsent {
    capture: AnonymousSourceCapture,
    libraries: Arc<[SuppliedBuildLibrary]>,
}

impl AnonymousSourceCapture {
    pub fn invocation_consent(
        &self,
        libraries: Arc<[SuppliedBuildLibrary]>,
    ) -> Arc<dyn InvocationSourceConsent> {
        if libraries.is_empty() {
            return Arc::new(self.clone());
        }
        Arc::new(SuppliedLibraryConsent {
            capture: self.clone(),
            libraries,
        })
    }
}

impl InvocationSourceConsent for SuppliedLibraryConsent {
    fn allows_selection(
        &self,
        sources: &Arc<SourceMap>,
        requester: SourceSpan,
        declaration: SourceSpan,
        purpose: DependencyScope,
    ) -> bool {
        self.capture
            .allows_selection(sources, requester, declaration, purpose)
    }

    fn allows_supplied_library(
        &self,
        sources: &Arc<SourceMap>,
        requester: SourceSpan,
        declaration: SourceSpan,
    ) -> bool {
        if !self
            .capture
            .allows_selection(sources, requester, requester, DependencyScope::Build)
        {
            return false;
        }
        let Some(file) = sources.file_at(declaration) else {
            return false;
        };
        if !sources.source_is_selected(declaration.source_id)
            || file.origin != source_admission::SourceOrigin::User
            // The receiving Build selected this file in its Build cohort. The
            // publisher reads a foreign declaration in its supplier's Product scope.
            || file.dependency_scope != DependencyScope::Build
            || declaration.span.start > declaration.span.end
            || declaration.span.end > file.source.len()
        {
            return false;
        }
        self.libraries.iter().any(|library| {
            let inputs = &library.dependency().inputs;
            file.package_identity == Some(inputs.root())
                && inputs.package_root(inputs.root()) == Some(file.package_root.as_path())
        })
    }
}
