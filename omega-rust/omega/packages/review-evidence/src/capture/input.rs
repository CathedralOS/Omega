/*
A review request borrows the source the product judgment actually accepted.
Equal-looking declarations from a different source owner cannot describe that
judgment. The input grants no policy acceptance; projections still owe complete
findings from this same retained compilation.
*/

use diagnostics::Diagnostic;
use products::CheckedCompilation;
use source_compiler::source_state::TypedTrees;

/// The source declarations available while preparing a policy. A completed
/// source projection owns its findings and carries no reference to this view.
#[derive(Clone, Copy)]
pub(super) struct PolicySource<'a> {
    candidate: &'a TypedTrees,
    accepted: &'a source_compiler::AcceptedTerminalSource,
    pub(super) publication: &'a source_compiler::SourcePublication,
    build: &'a products::ComputedBuildConfig,
    pub(super) admission: &'a products::BuildAdmission,
    sources: &'a package_compilation::CheckedSourceCustody,
}

impl<'a> PolicySource<'a> {
    fn new(
        program: &'a std::sync::Arc<source_compiler::source_state::FrozenTypedTrees>,
        candidate: &'a TypedTrees,
        publication: &'a source_compiler::SourcePublication,
        build: &'a products::ComputedBuildConfig,
        admission: &'a products::BuildAdmission,
        sources: &'a package_compilation::CheckedSourceCustody,
    ) -> Result<Self, Vec<Diagnostic>> {
        sources
            .require_owner(program)
            .map_err(|error| vec![error])?;
        publication
            .require_source_owner(program)
            .map_err(|error| vec![error])?;
        if !std::ptr::eq(candidate, program.as_ref().as_ref()) {
            return Err(vec![Diagnostic::error(
                "package review candidate differs from the accepted source owner",
            )]);
        }
        let accepted = publication.accepted()?;
        if accepted.is_invocation() {
            return Err(vec![Diagnostic::error(
                "package review requires a portable package judgment",
            )]);
        }
        let package = sources.package_identity();
        if package.is_none() || publication.roster()?.subject != package {
            return Err(vec![Diagnostic::error(
                "package review requires the accepted package's complete declaration subject",
            )]);
        }
        if build.selected_build_machine_symbol.is_some() != build.accepted_build.is_some() {
            return Err(vec![Diagnostic::error(
                "package review requires the selected Build's accepted judgment",
            )]);
        }
        Ok(Self {
            candidate,
            accepted,
            publication,
            build,
            admission,
            sources,
        })
    }

    pub(super) fn product(
        source: &'a products::CheckedProductSource,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::new(
            &source.program,
            &source.program,
            &source.terminal_publication,
            &source.computed_build_config,
            &source.admission,
            &source.sources,
        )
    }

    pub(super) fn typed(&self) -> &'a TypedTrees {
        self.candidate
    }
    pub(super) fn accepted(&self) -> &'a source_compiler::AcceptedTerminalSource {
        self.accepted
    }
    pub(super) fn sources(&self) -> &'a package_compilation::CheckedSourceCustody {
        self.sources
    }
    pub(super) fn build(&self) -> &'a products::ComputedBuildConfig {
        self.build
    }
}

#[derive(Clone, Copy)]
pub struct PackageReviewInput<'a> {
    pub(super) source: PolicySource<'a>,
    custody: &'a CheckedCompilation,
}

impl<'a> PackageReviewInput<'a> {
    pub fn supplied(
        candidate: &'a TypedTrees,
        custody: &'a CheckedCompilation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let source = PolicySource::new(
            custody.source_inspection().program(),
            candidate,
            custody.terminal_publication(),
            custody.build(),
            custody.build_admission(),
            custody.sources(),
        )?;
        Ok(Self { source, custody })
    }

    pub fn candidate(&self) -> &'a TypedTrees {
        self.source.typed()
    }
    pub fn compilation(&self) -> &'a CheckedCompilation {
        self.custody
    }

    pub fn module(&self) -> Result<&'a terminal_psi::TerminalModule, Vec<Diagnostic>> {
        Ok(self.source.accepted().module())
    }
}

pub trait IntoPackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>>;
}

impl<'a> IntoPackageReviewInput<'a> for &'a CheckedCompilation {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        PackageReviewInput::supplied(self, self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for PackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        Ok(self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for &PackageReviewInput<'a> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        Ok(*self)
    }
}

impl<'a> IntoPackageReviewInput<'a> for Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
    fn into_review_input(self) -> Result<PackageReviewInput<'a>, Vec<Diagnostic>> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_review_input_preserves_the_full_projection_error() {
        let input: Result<PackageReviewInput<'static>, Vec<Diagnostic>> =
            Err(vec![Diagnostic::error("missing route contract custody")]);
        let errors = match input.into_review_input() {
            Ok(_) => panic!("an unavailable input cannot produce a review namespace"),
            Err(errors) => errors,
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "missing route contract custody");
    }

    #[test]
    fn policy_projection_propagates_the_complete_input_refusal() {
        let input: Result<PackageReviewInput<'static>, Vec<Diagnostic>> =
            Err(vec![Diagnostic::error("exact missing-custody reason")]);
        let error = super::super::project_checked_package_policy(
            input,
            target::TargetProfile::host(),
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error[0].message, "exact missing-custody reason");
    }
}
