//! Legacy evidence reconstruction is unavailable after source-checker removal.

use crate::capture::PackageReviewInput;
use crate::record::PackagePolicyCallables;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(crate) fn project_checked_callable_policy<'a>(
    _input: impl Into<PackageReviewInput<'a>>,
    _target: TargetProfile,
    _package: PackageKeyIdentity,
) -> Result<PackagePolicyCallables, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package-review callable policy evidence",
    )])
}
