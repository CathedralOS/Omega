//! Accepted publication and execution custody project into inert review data.
//! Review does not run a second source checker or reconstruct its facts.

mod assumed_bindings;
mod authority;
mod framing;
mod lifetime_topology;
mod native_bindings;
mod native_parameters;
mod nominal;
mod package_review;
mod physical_geometry;
mod physical_plan;
mod published_dependencies;
mod published_input;
mod published_projection;
mod published_providers;
mod published_services;
mod runtime_permissions;
mod semantic_bindings;

pub use package_review::{PackageReviewProjectionError, project_package_review};
pub use published_input::{
    PublishedPackageReviewInput, project_checked_package_policy,
    project_checked_terminal_permission_policy,
};
pub use runtime_permissions::PreparedRuntimePolicy;
pub use semantic_bindings::{
    SemanticBindingProjectionError, candidate_hosted_exit_bindings, candidate_service_bindings,
    candidate_target_entry_binding,
};

pub(crate) trait EvidenceOption<T> {
    fn or_evidence(self, message: impl Into<String>) -> Result<T, Vec<diagnostics::Diagnostic>>;
}
impl<T> EvidenceOption<T> for Option<T> {
    fn or_evidence(self, message: impl Into<String>) -> Result<T, Vec<diagnostics::Diagnostic>> {
        self.ok_or_else(|| vec![diagnostics::Diagnostic::error(message)])
    }
}
