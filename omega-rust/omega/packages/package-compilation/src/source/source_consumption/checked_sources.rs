// Pre-build (`BuildSourceCustody`) and final checked (`CheckedSourceCustody`) source-consumption custody.
//
// `BuildSourceCustody` tracks the frontend's source file count, admitted build
// execution profile, dependency-generated source custody, and base source-consumption
// commitment across pre-build admission and `admit_generated_extension`. Once the
// program passes semantic checking (`CheckedTrees`), `BuildSourceCustody::seal`
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
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceCustody {
    pub source_file_count: usize,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSourceCustody {
    build: BuildSourceCustody,
    package_purpose: Option<build_declarations::DependencyPurpose>,
    package_subject: Option<PackageCompilationSubject>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
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
        program: &TypedTrees,
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
            build: self,
            package_purpose: package_inputs.map(PackageCompilationInputs::compilation_purpose),
            package_subject,
            exact_toolchain_sources,
        })
    }
}

impl CheckedSourceCustody {
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
