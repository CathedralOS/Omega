//! D29 compiler associations normalized without reconstruction receipts.

mod demands;
mod realizations;

use crate::package_evidence::capture::PackageReviewInput;
use crate::package_evidence::record::PackagePolicyBoundaryApplications;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// Retain exact open demands and closed selected relationships, not coverage.
/// Plan coordinates use the same canonical order as selected-provider policy.
pub fn project_checked_boundary_application_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBoundaryApplications, Vec<Diagnostic>> {
    let compilation = &input.into();
    let (providers, indices) = super::policy::project_with_indices(compilation, target, package)?;
    let policy = PackagePolicyBoundaryApplications {
        demands: demands::project(compilation, package)?,
        realizations: realizations::project(compilation, package, &providers, &indices)?,
    };
    policy
        .validate_canonical_structure(package, target, &providers)
        .map_err(rejected)?;
    Ok(policy)
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "boundary application policy rejects {reason}"
    ))]
}
