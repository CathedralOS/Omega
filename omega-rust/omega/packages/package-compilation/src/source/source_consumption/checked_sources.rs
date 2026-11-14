// Pre-build (`BuildSourceCustody`) and final checked (`CheckedSourceCustody`) source-consumption custody.
//
// `BuildSourceCustody` tracks the frontend's source file count, admitted build
// execution profile, dependency-generated source custody, and base source-consumption
// commitment across pre-build admission and `admit_generated_extension`. Once the
// immutable typed source is retained, `BuildSourceCustody::seal`
// derives the final `PackageCompilationSubject` and `exact_toolchain_sources`
// without reopening the host filesystem. `package_build_handoff` exports only the
// activation's own generated sources (`own_generated_sources`) bound to the
// compilation's `PackageSourceConsumptionCommitment`.

use super::{
    PackageCompilationSubject, PackageSourceConsumptionCommitment,
    derive_package_compilation_subject, toolchain_source_identities,
};
use crate::{PackageBuildHandoff, PackageCompilationInputs};
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::{FrozenTypedTrees, ProductSourceIdentity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceCustody {
    pub source_file_count: usize,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSourceCustody {
    owner: ProductSourceIdentity,
    build: BuildSourceCustody,
    package_purpose: Option<build_declarations::DependencyPurpose>,
    package_subject: Option<PackageCompilationSubject>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
    semantic_bindings: Vec<crate::AcceptedSemanticBinding>,
}

impl BuildSourceCustody {
    pub fn package_build_handoff(
        &self,
        program: &typed_trees::TypedTrees,
        inputs: &PackageCompilationInputs,
        target: target::TargetProfile,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageBuildHandoff, Vec<Diagnostic>> {
        let subject = derive_package_compilation_subject(
            program.symbols.source_files(),
            inputs,
            &self.generated_source_custody,
        )?;
        Ok(PackageBuildHandoff::from_source_consumption(
            subject.root(),
            inputs.compilation_purpose(),
            target,
            self.build_execution_profile,
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            own_generated_sources,
        ))
    }

    pub fn admit_generated_extension<'inputs>(
        &mut self,
        base_sources: &std::sync::Arc<source::SourceMap>,
        package_inputs: Option<&'inputs PackageCompilationInputs>,
        generated_sources: &[build_output::PackageGeneratedSource],
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<
        Option<(
            crate::source::source_imports::GeneratedSourceExtension,
            &'inputs PackageCompilationInputs,
        )>,
        Vec<Diagnostic>,
    > {
        if generated_sources.is_empty() {
            return Ok(None);
        }
        let package_inputs = package_inputs.ok_or_else(|| {
            vec![Diagnostic::error(
                "generated-source final compilation requires package-aware source custody",
            )]
        })?;
        let package_root = package_inputs
            .package_root(package_inputs.root())
            .expect("validated package inputs retain their root package");
        let extension = crate::source::source_imports::admit_generated_extension(
            base_sources,
            package_root,
            Some(package_inputs.root()),
            generated_sources,
            catalogue,
        )?;
        let source_file_count = self
            .source_file_count
            .checked_add(extension.custody().len())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "generated package source count exceeds the compiler range",
                )]
            })?;
        self.generated_source_custody
            .extend(extension.custody().iter().cloned());
        self.source_file_count = source_file_count;
        Ok(Some((extension, package_inputs)))
    }

    pub fn seal(
        self,
        program: &Arc<FrozenTypedTrees>,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<CheckedSourceCustody, Vec<Diagnostic>> {
        let package_subject = package_inputs
            .map(|inputs| {
                derive_package_compilation_subject(
                    program.symbols.source_files(),
                    inputs,
                    &self.generated_source_custody,
                )
            })
            .transpose()?;
        let exact_toolchain_sources = toolchain_source_identities(program.symbols.source_files())?;
        Ok(CheckedSourceCustody {
            owner: ProductSourceIdentity::new(program),
            build: self,
            package_purpose: package_inputs.map(PackageCompilationInputs::compilation_purpose),
            package_subject,
            exact_toolchain_sources,
            semantic_bindings: package_inputs
                .map(|inputs| inputs.accepted_semantic_bindings().cloned().collect())
                .unwrap_or_default(),
        })
    }
}

impl CheckedSourceCustody {
    pub fn accepted_semantic_binding(
        &self,
        role: crate::AcceptedSemanticBindingRole,
    ) -> Option<&crate::AcceptedSemanticBinding> {
        self.semantic_bindings
            .iter()
            .find(|binding| binding.role() == role)
    }

    pub fn accepted_semantic_bindings(
        &self,
    ) -> impl Iterator<Item = &crate::AcceptedSemanticBinding> {
        self.semantic_bindings.iter()
    }
    /// Source consumption records do not certify semantic acceptance. Their
    /// exact owner must still match the source the route independently judged.
    pub fn require_owner(&self, program: &Arc<FrozenTypedTrees>) -> Result<(), Diagnostic> {
        if !self.owner.is_for(program) {
            return Err(Diagnostic::error(
                "source consumption custody belongs to another typed source owner",
            ));
        }
        Ok(())
    }

    pub fn source_identity(&self) -> &ProductSourceIdentity {
        &self.owner
    }

    pub const fn source_file_count(&self) -> usize {
        self.build.source_file_count
    }

    pub const fn package_subject(&self) -> Option<&PackageCompilationSubject> {
        self.package_subject.as_ref()
    }

    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match self.package_subject.as_ref() {
            Some(subject) => Some(subject.root()),
            None => None,
        }
    }

    pub const fn dependency_closure(&self) -> Option<&crate::PackageDependencyClosure> {
        match self.package_subject.as_ref() {
            Some(subject) => Some(subject.dependency_closure()),
            None => None,
        }
    }

    pub const fn source_consumption_commitment(
        &self,
    ) -> Option<PackageSourceConsumptionCommitment> {
        match self.package_subject.as_ref() {
            Some(subject) => Some(subject.source_consumption_commitment()),
            None => None,
        }
    }

    pub const fn base_source_consumption_commitment(
        &self,
    ) -> Option<PackageSourceConsumptionCommitment> {
        self.build.base_source_consumption_commitment
    }

    pub fn exact_toolchain_sources(&self) -> &[(source::SourceId, [u8; 32])] {
        &self.exact_toolchain_sources
    }

    pub fn package_build_handoff(
        &self,
        selected_target: Option<target::TargetProfile>,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageBuildHandoff, &'static str> {
        let subject = self
            .package_subject
            .as_ref()
            .ok_or("generated-source bundles require package-aware compilation")?;
        let target =
            selected_target.ok_or("generated-source bundles require one selected target")?;
        Ok(PackageBuildHandoff::from_checked(
            subject.root(),
            self.package_purpose
                .ok_or("generated-source bundles require a checked package purpose")?,
            target,
            self.build.build_execution_profile,
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            own_generated_sources,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> BuildSourceCustody {
        BuildSourceCustody {
            source_file_count: 0,
            build_execution_profile: None,
            generated_source_custody: Vec::new(),
            base_source_consumption_commitment: None,
        }
    }

    #[test]
    fn source_seal_binds_the_exact_frozen_owner_without_claiming_a_verdict() {
        let program = Arc::new(FrozenTypedTrees::from(typed_trees::TypedTrees::default()));
        let other = Arc::new(program.as_ref().clone());
        let seal = build().seal(&program, None).unwrap();
        assert!(seal.require_owner(&program).is_ok());
        assert!(seal.require_owner(&other).is_err());
        assert_eq!(seal, seal.clone());
        assert_ne!(seal, build().seal(&other, None).unwrap());
        assert!(seal.package_subject().is_none());
        assert!(seal.exact_toolchain_sources().is_empty());
    }
}
