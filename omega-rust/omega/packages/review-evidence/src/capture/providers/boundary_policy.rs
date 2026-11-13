// Receipt-free boundary-application policy projection retaining open demands and
// closed selected relationships. Plan coordinates come from the selected-provider
// policy produced by this capture so service signatures and provider checks are not
// rebuilt a second time.

mod demands;
mod realizations;

use crate::capture::PackageReviewInput;
use crate::capture::refusal;
use crate::record::PackagePolicyBoundaryApplications;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(crate) fn project_checked_boundary_application_policy<'a>(
    input: impl crate::IntoPackageReviewInput<'a>,
    package: PackageKeyIdentity,
    original_indices: &[usize],
) -> Result<PackagePolicyBoundaryApplications, Vec<Diagnostic>> {
    let input = input.into_review_input()?;
    let projected = input
        .external_providers
        .is_none()
        .then(|| input.project_external_providers())
        .transpose()?;
    let services = input
        .service_declarations
        .is_none()
        .then(|| input.project_service_declarations())
        .transpose()?;
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
