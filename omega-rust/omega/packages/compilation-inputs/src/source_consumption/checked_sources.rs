//! Seal a compilation's source consumption and retain its generated-source handoff.

use super::{
    PackageCompilationSubject, PackageSourceConsumptionCommitment,
    derive_package_compilation_subject, toolchain_source_identities,
};
use crate::{PackageCompilationInputs, PackageGeneratedSourceBundle};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

/// Source custody spanning pre-build admission and the generated extension.
/// These are continuation inputs, not evidence of a final checked closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSourceCustody {
    pub source_file_count: usize,
    pub build_execution_profile: Option<target::TargetProfile>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    pub base_source_consumption_commitment: Option<PackageSourceConsumptionCommitment>,
}

/// Exact source evidence sealed against one final checked program. The package
/// owner derives identities, checks live custody, and constructs dependency
/// handoffs; the compiler retains this result without assembling its fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedSourceCustody {
    build: BuildSourceCustody,
    package_purpose: Option<build_declarations::DependencyPurpose>,
    package_subject: Option<PackageCompilationSubject>,
    exact_toolchain_sources: Vec<(source::SourceId, [u8; 32])>,
}

impl BuildSourceCustody {
    /// Seal a completed Build's generated handoff against its retained typed
    /// continuation. This establishes source and staged-output custody only;
    /// it does not claim the dependency's product has passed final checking.
    /// Consumers still check these bytes in their own product or Build scope.
    pub fn package_generated_source_bundle(
        &self,
        program: &typed_trees::TypedTrees,
        inputs: &PackageCompilationInputs,
        target: target::TargetProfile,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageGeneratedSourceBundle, Vec<Diagnostic>> {
        let subject = derive_package_compilation_subject(
            program.symbols.source_files(),
            inputs,
            &self.generated_source_custody,
        )?;
        Ok(PackageGeneratedSourceBundle::from_source_consumption(
            subject.root(),
            inputs.compilation_purpose(),
            target,
            self.build_execution_profile,
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            own_generated_sources,
        ))
    }

    /// Admit this activation's generated sources and retain their exact source
    /// coordinates before parsing. Empty output needs no package context.
    /// Rejection leaves the prior count and custody unchanged; admission alone
    /// does not seal a checked source closure.
    pub fn admit_generated_extension<'inputs>(
        &mut self,
        base_sources: &std::sync::Arc<source::SourceMap>,
        package_inputs: Option<&'inputs PackageCompilationInputs>,
        generated_sources: &[build_output::PackageGeneratedSource],
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<
        Option<(
            crate::source_imports::GeneratedSourceExtension,
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
        let extension = crate::source_imports::admit_generated_extension(
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

    /// Preserve the source-admission order: derive the final package subject,
    /// then identify toolchain sources. Neither step reopens the host: the
    /// frontend read each consumed file once and this seal commits to those
    /// bytes, so there is nothing here for a second read to establish.
    pub fn seal(
        self,
        program: &CheckedTrees,
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

    /// Reconciled root package identity of a package-aware compilation.
    /// Standalone compilation has no package identity.
    pub const fn package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match self.package_subject.as_ref() {
            Some(subject) => Some(subject.root()),
            None => None,
        }
    }

    /// Exact source-path-free dependency closure consumed by a package-aware
    /// compilation. Standalone compilation has no package closure.
    pub const fn dependency_closure(&self) -> Option<&crate::PackageDependencyClosure> {
        match self.package_subject.as_ref() {
            Some(subject) => Some(subject.dependency_closure()),
            None => None,
        }
    }

    /// Canonical commitment to the exact source bytes a package-aware
    /// compilation consumed. Standalone compilation has none.
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

    /// Export only the activation's own included sources. The final checked
    /// closure also contains dependency-generated sources; those establish the
    /// subject but must not be re-exported as this package's generated output.
    /// The caller supplies the evaluated Build's own source roster and selected
    /// product target, which remain distinct from its build execution profile.
    pub fn package_generated_source_bundle(
        &self,
        selected_target: Option<target::TargetProfile>,
        own_generated_sources: Vec<build_output::PackageGeneratedSource>,
    ) -> Result<PackageGeneratedSourceBundle, &'static str> {
        let subject = self
            .package_subject
            .as_ref()
            .ok_or("generated-source bundles require package-aware compilation")?;
        let target =
            selected_target.ok_or("generated-source bundles require one selected target")?;
        Ok(PackageGeneratedSourceBundle::from_checked(
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
