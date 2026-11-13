//! Legacy evidence reconstruction is unavailable after source-checker removal.

use crate::record::PackagePolicyCallables;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(crate) fn project_checked_callable_policy<'a>(
    input: impl crate::IntoPackageReviewInput<'a>,
    _target: TargetProfile,
    _package: PackageKeyIdentity,
) -> Result<PackagePolicyCallables, Vec<Diagnostic>> {
    input.into_review_input()?;
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package-review callable policy evidence",
    )])
}
