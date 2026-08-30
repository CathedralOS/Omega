//! Complete checked package-review projection operation.
//!
//! The entrance validates compiler custody, collects independently reviewable
//! surfaces, and delegates canonical source assembly without owning any of
//! those projection details itself.

mod assembly;
mod callables;
mod providers;
mod surface;
mod validation;

use self::assembly::PendingPackageReview;
use self::callables::project_package_callables;
use self::providers::project_selected_providers;
use self::surface::project_package_surface;
use self::validation::validate_review_compilation;
use super::authority::{project_dangerous_authorities, project_dangerous_authority_slack};
use crate::record::CheckedPackageReviewProjection;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

/// Project the exact checked authority facts that are already safely joined.
///
/// This refuses standalone and target-free compilations, missing checked fact
/// rows, and a non-root build machine. Compiler-generated nominals inherit the
/// exact authored source provenance of their mandatory derivation origin.
/// Truly source-free nominals remain explicit `Unresolved` review rows; a later
/// admission certificate must reject them rather than treating them as empty
/// authority.
pub fn project_checked_package_review(
    compilation: &CheckedCompilation,
) -> Result<CheckedPackageReviewProjection, Vec<Diagnostic>> {
    let (package, target) = validate_review_compilation(compilation)?;
    let surface = project_package_surface(compilation, package)?;
    let callables = project_package_callables(compilation, package)?;
    let dangerous_authorities = project_dangerous_authorities(compilation, &callables.callables)?;
    let dangerous_authority_slack =
        project_dangerous_authority_slack(compilation, &callables.callables)?;
    let providers = project_selected_providers(compilation, target)?;

    PendingPackageReview {
        package,
        target,
        surface,
        callables,
        dangerous_authorities,
        dangerous_authority_slack,
        providers,
    }
    .finalize(compilation)
}
