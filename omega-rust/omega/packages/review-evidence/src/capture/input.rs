/*
A review request borrows genuine typed source and compilation metadata; it has no
checking-input implementation or old fact namespace. Construction remains
unavailable until the route publishes review contract custody. Every projection
preserves that refusal rather than manufacturing an empty policy.
*/

use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Clone, Copy)]
pub struct PackageReviewInput<'a> {
    candidate: &'a TypedTrees,
    custody: &'a CheckedCompilation,
}

impl<'a> PackageReviewInput<'a> {
    pub fn supplied(
        _candidate: &'a TypedTrees,
        _custody: &'a CheckedCompilation,
    ) -> Result<Self, Vec<Diagnostic>> {
        Err(super::unavailable_projection("package review"))
    }

    pub fn candidate(&self) -> &'a TypedTrees {
        self.candidate
    }

    pub fn compilation(&self) -> &'a CheckedCompilation {
        self.custody
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
