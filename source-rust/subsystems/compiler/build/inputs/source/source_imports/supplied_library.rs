//! The library a caller supplied for one Build before its first statement.

use super::ProductQueryDependency;
use crate::PackageCompilationInputs;
use build_declarations::{
    BuildDeclarationKind, BuildDependencySource, BuildRequest, DependencyPurpose, ProjectName,
};

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

    pub fn request(&self) -> &BuildRequest {
        &self.request
    }
    pub fn dependency(&self) -> &ProductQueryDependency {
        &self.dependency
    }

    /// An identical package key can select different captured bytes on two invocations.
    pub fn validate_join(&self, joined: &PackageCompilationInputs) -> Result<(), String> {
        let library = &self.dependency.inputs;
        if joined.dependency_target_for_purpose(
            joined.root(),
            DependencyPurpose::Build,
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
