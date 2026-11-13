//! Review borrows the accepted Product and its exact immutable source owner.
//! A caller-supplied typed tree cannot replace the publication's source.

use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;
use typed_trees::TypedTrees;

#[derive(Clone, Copy)]
pub struct PublishedPackageReviewInput<'a> {
    pub(super) candidate: &'a TypedTrees,
    pub(super) custody: &'a CheckedCompilation,
}
impl<'a> PublishedPackageReviewInput<'a> {
    pub fn supplied(candidate: &'a TypedTrees, custody: &'a CheckedCompilation) -> Self {
        Self { candidate, custody }
    }
    pub(super) fn require_source(
        &self,
        target: TargetProfile,
        package: PackageKeyIdentity,
    ) -> Result<(), Vec<Diagnostic>> {
        let source = self.custody.source_inspection().program();
        if !std::ptr::eq(self.candidate, &***source.typed()) {
            return Err(rejected(
                "candidate has no exact published source and proof custody",
            ));
        }
        if self.custody.sources().package_identity() != Some(package)
            || self.custody.execution().selected_target_profile != Some(target)
            || self.custody.execution().selected_native_target() != Some(target.native_target())
        {
            return Err(rejected(
                "package or target differs from the published root activation",
            ));
        }
        Ok(())
    }
}
impl<'a> From<&'a CheckedCompilation> for PublishedPackageReviewInput<'a> {
    fn from(custody: &'a CheckedCompilation) -> Self {
        Self::supplied(&***custody.source_inspection().program().typed(), custody)
    }
}
impl<'a> From<&PublishedPackageReviewInput<'a>> for PublishedPackageReviewInput<'a> {
    fn from(input: &PublishedPackageReviewInput<'a>) -> Self {
        *input
    }
}

pub fn project_checked_package_policy<'a>(
    input: impl Into<PublishedPackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyBaseline, Vec<Diagnostic>> {
    let input = input.into();
    input.require_source(target, package)?;
    super::published_projection::package(input, target, package)
}

pub fn project_checked_terminal_permission_policy<'a>(
    input: impl Into<PublishedPackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    let input = input.into();
    input.require_source(target, package)?;
    super::published_projection::permissions(input, target, package)
}

pub(crate) fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "published package review: {reason}"
    ))]
}
