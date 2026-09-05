//! D29 compiler associations normalized without reconstruction receipts.

mod demands;
mod realizations;

use crate::record::PackagePolicyBoundaryApplications;
use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

/// Retain exact open demands and closed selected relationships, not coverage.
/// Plan coordinates use the same canonical order as selected-provider policy.
pub fn project_checked_boundary_application_policy(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBoundaryApplications, Vec<Diagnostic>> {
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
