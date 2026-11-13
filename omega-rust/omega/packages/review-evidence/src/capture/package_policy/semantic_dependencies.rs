//! Legacy evidence reconstruction is unavailable after source-checker removal.

use crate::capture::PackageReviewInput;
use crate::record::{PackagePolicyCallables, PackagePolicySemanticDependency};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(super) fn project(
    _compilation: &PackageReviewInput<'_>,
    _package: PackageKeyIdentity,
    _callables: &PackagePolicyCallables,
) -> Result<Vec<PackagePolicySemanticDependency>, Vec<Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: package-review semantic dependency evidence",
    )])
}
