/*
Portable review records and their encodings survive the old checker's removal.
The route does not yet publish the contract custody needed to project policies,
so these entrances refuse explicitly. Source/package metadata and typed binding
nominations remain independent of a policy verdict.
*/

mod input;
mod package_review;
mod runtime_permissions;
mod semantic_bindings;

pub use input::{IntoPackageReviewInput, PackageReviewInput};
pub use package_review::{PackageReviewProjectionError, project_package_review};
pub use runtime_permissions::PreparedRuntimePolicy;
pub use semantic_bindings::{
    SemanticBindingProjectionError, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};

use diagnostics::Diagnostic;

pub(super) fn unavailable_projection(purpose: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "no product route yet: {purpose} requires route-owned contract custody"
    ))]
}

pub fn project_checked_package_policy<'a>(
    input: impl IntoPackageReviewInput<'a>,
    _target: target::TargetProfile,
    _package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyBaseline, Vec<Diagnostic>> {
    input.into_review_input()?;
    Err(unavailable_projection("package policy projection"))
}

pub fn project_checked_terminal_permission_policy<'a>(
    input: impl IntoPackageReviewInput<'a>,
    _target: target::TargetProfile,
    _package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    input.into_review_input()?;
    Err(unavailable_projection("terminal permission projection"))
}
