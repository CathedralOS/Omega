//! Review inputs retain the accepted module and its exact immutable source
//! owner. Legacy checker queries cannot supply missing publication receipts.

use compilation_products::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;
use typed_trees::TypedTrees;

#[derive(Clone, Copy)]
pub struct PublishedPackageReviewInput<'a> {
    source: ReviewSource<'a>,
}
#[derive(Clone, Copy)]
enum ReviewSource<'a> {
    Published {
        candidate: &'a TypedTrees,
        custody: &'a CheckedCompilation,
    },
    Legacy(super::PackageReviewInput<'a>),
}
impl<'a> PublishedPackageReviewInput<'a> {
    pub fn supplied(candidate: &'a TypedTrees, custody: &'a CheckedCompilation) -> Self {
        Self {
            source: ReviewSource::Published { candidate, custody },
        }
    }
    fn require_source(
        &self,
        target: TargetProfile,
        package: PackageKeyIdentity,
    ) -> Result<(), Vec<Diagnostic>> {
        let ReviewSource::Published { candidate, custody } = self.source else {
            return Err(vec![Diagnostic::error(
                "published review input has no accepted source owner",
            )]);
        };
        let source = custody.source_inspection().program();
        if !std::ptr::eq(candidate, &***source.typed()) {
            return Err(vec![Diagnostic::error(
                "review candidate has no exact published source and proof custody",
            )]);
        }
        if custody.sources().package_identity() != Some(package)
            || custody.execution().selected_target_profile != Some(target)
            || custody.execution().selected_native_target() != Some(target.native_target())
        {
            return Err(vec![Diagnostic::error(
                "review package or target differs from the published root activation",
            )]);
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
    if let ReviewSource::Legacy(source) = input.source {
        return super::package_policy::project_source_package_policy(source, target, package);
    }
    input.require_source(target, package)?;
    Err(vec![Diagnostic::error(
        "published package review needs exact public API, contract, data and behavior proof receipts; legacy checker facts cannot replace them",
    )])
}

pub fn project_checked_terminal_permission_policy<'a>(
    input: impl Into<PublishedPackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    let input = input.into();
    if let ReviewSource::Legacy(source) = input.source {
        return super::terminal_authority_permissions::project_source_terminal_permission_policy(
            source, target, package,
        );
    }
    input.require_source(target, package)?;
    Err(vec![Diagnostic::error(
        "published terminal-permission disclosure needs the complete named service contract receipt roster",
    )])
}

impl<'a> From<super::PackageReviewInput<'a>> for PublishedPackageReviewInput<'a> {
    fn from(input: super::PackageReviewInput<'a>) -> Self {
        Self {
            source: ReviewSource::Legacy(input),
        }
    }
}
