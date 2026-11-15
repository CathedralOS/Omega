/*
A review request borrows the source the product judgment actually accepted.
Equal-looking declarations from a different source owner cannot describe that
judgment. The input grants no policy acceptance; projections still owe complete
findings from this same retained compilation.
*/

use diagnostics::Diagnostic;
use products::CheckedCompilation;
use typed_trees::TypedTrees;

#[derive(Clone, Copy)]
pub struct PackageReviewInput<'a> {
    candidate: &'a TypedTrees,
    custody: &'a CheckedCompilation,
}

impl<'a> PackageReviewInput<'a> {
    pub fn supplied(
        candidate: &'a TypedTrees,
        custody: &'a CheckedCompilation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let source = custody.source_inspection().program();
        custody
            .sources()
            .require_owner(source)
            .map_err(|error| vec![error])?;
        custody
            .terminal_publication()
            .require_source_owner(source)
            .map_err(|error| vec![error])?;
        if !std::ptr::eq(candidate, source.as_ref().as_ref()) {
            return Err(vec![Diagnostic::error(
                "package review candidate differs from the accepted source owner",
            )]);
        }
        let accepted = custody.terminal_publication().accepted()?;
        if accepted.is_invocation() {
            return Err(vec![Diagnostic::error(
                "package review requires a portable package judgment",
            )]);
        }
        let package = custody.sources().package_identity();
        if package.is_none() || custody.terminal_publication().roster()?.subject != package {
            return Err(vec![Diagnostic::error(
                "package review requires the accepted package's complete declaration subject",
            )]);
        }
        if custody.build().selected_build_machine_symbol.is_some()
            != custody.build().accepted_build.is_some()
        {
            return Err(vec![Diagnostic::error(
                "package review requires the selected Build's accepted judgment",
            )]);
        }
        Ok(Self { candidate, custody })
    }

    pub fn candidate(&self) -> &'a TypedTrees {
        self.candidate
    }

    pub fn compilation(&self) -> &'a CheckedCompilation {
        self.custody
    }

    /// Read the exact module whose judgment admitted this source, before
    /// projecting its contracts or effects into inert review rows.
    pub fn module(&self) -> Result<&'a terminal_psi::TerminalModule, Vec<Diagnostic>> {
        Ok(self.custody.terminal_publication().accepted()?.module())
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
