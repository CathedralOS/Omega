//! D29 compiler associations normalized without reconstruction receipts.

mod demands;
mod realizations;

use crate::capture::PackageReviewInput;
use crate::capture::refusal;
use crate::record::PackagePolicyBoundaryApplications;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

/// Retain exact open demands and closed selected relationships, not coverage.
/// Plan coordinates come from the selected-provider policy produced by this
/// capture. Rebuilding that policy here would duplicate every service signature
/// and provider check merely to recover the same canonical ordering.
pub(crate) fn project_checked_boundary_application_policy<'a>(
    input: impl Into<PackageReviewInput<'a>>,
    package: PackageKeyIdentity,
    original_indices: &[usize],
) -> Result<PackagePolicyBoundaryApplications, Vec<Diagnostic>> {
    let input = input.into();
    let projected = input
        .external_providers
        .is_none()
        .then(|| input.project_external_providers())
        .transpose()?;
    let services = input
        .service_declarations
        .is_none()
        .then(|| input.project_service_declarations());
    let compilation = &PackageReviewInput {
        service_declarations: input.service_declarations.or(services.as_ref()),
        external_providers: input.external_providers.or(projected.as_ref()),
        ..input
    };
    let policy = PackagePolicyBoundaryApplications {
        demands: demands::project(compilation, package)?,
        realizations: realizations::project(compilation, package, original_indices)?,
    };
    Ok(policy)
}

refusal!(reason, "boundary application policy rejects {reason}");
