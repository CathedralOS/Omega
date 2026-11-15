/*
Review borrows the exact source and module retained by Product judgment.
Supported declaration inventories produce inert policy rows; an unsupported
nonempty contract or authority inventory prevents a partial review capsule.
*/

mod callables;
mod declarations;
mod input;
mod package_review;
mod policy;
mod providers;
mod runtime_permissions;
mod semantic_bindings;

pub use input::{IntoPackageReviewInput, PackageReviewInput};
pub use package_review::{PackageReviewProjectionError, project_package_review};
pub use runtime_permissions::PreparedRuntimePolicy;
pub use semantic_bindings::{SemanticBindingProjectionError, candidate_target_entry_binding};

use diagnostics::Diagnostic;

pub fn project_checked_package_policy<'a>(
    input: impl IntoPackageReviewInput<'a>,
    target: target::TargetProfile,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyBaseline, Vec<Diagnostic>> {
    policy::project(input.into_review_input()?, target, package)
}

pub fn project_checked_terminal_permission_policy<'a>(
    input: impl IntoPackageReviewInput<'a>,
    target: target::TargetProfile,
    package: semantic_vocabulary::PackageKeyIdentity,
) -> Result<crate::record::PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    Ok(policy::project(input.into_review_input()?, target, package)?.terminal_permissions)
}
