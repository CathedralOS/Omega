//! Resolve immutable source custody into one declared package source.
//!
//! The entry points are grouped by source lineage. Shared declaration
//! projection, errors, and resolved custody live behind this narrow facade.

mod custody;
mod error;
pub(crate) mod git;
mod local;
mod materialization;
mod navigation;
mod projection;
mod resolved;
mod selection;
mod workspace;
pub(crate) mod workspace_path;

pub use custody::PackageSourceCustody;
pub(crate) use custody::{CHECKED_SOURCE_CACHE_LANE, COMPILED_REVIEW_CACHE_LANE};
pub use error::ResolvePackageSourceError;
pub(crate) use git::resolve_selected_git_package_source_from_pin_in_lanes;
pub(crate) use git::resolve_selected_git_project_source_from_pin_in_lanes;
pub(crate) use git::workspace::{GitWorkspaceSelectionDeclarations, GitWorkspaceSelectionEvidence};
pub use git::{
    GitPackageSourceRequest, resolve_git_package_source, resolve_selected_git_package_source,
    resolve_selected_git_project_source,
};
pub(crate) use git::{
    resolve_selected_git_package_source_at_revision_in_lanes,
    resolve_selected_git_project_source_at_revision_in_lanes,
};
pub use local::{
    bind_staged_external_local_project_source, resolve_external_local_package_source,
    resolve_external_local_project_source,
};
pub(crate) use local::{
    capture_external_local_project_source, recover_cached_external_local_source,
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
};
pub use materialization::PackageSourceMaterialization;
pub use navigation::PackageSourceNavigation;
pub use resolved::ResolvedPackageSource;
pub use selection::{PackageSourceSelectionEvidence, PackageSourceSelectionEvidenceError};
pub(crate) use workspace::resolve_workspace_member_package_source_in_lane;
pub(crate) use workspace::resolve_workspace_member_project_source_in_lane;
pub use workspace::{
    resolve_workspace_member_package_source, resolve_workspace_member_project_source,
};

/// The accessor surface every resolved-package-source record shares:
/// custody and resolved shapes carry identically-named projection fields and
/// expose the same dependency-request delegation.
macro_rules! package_source_surface_accessors {
    () => {
        pub fn key(&self) -> &PackageKey {
            &self.key
        }

        pub const fn role(&self) -> BuildDeclarationKind {
            self.role
        }

        pub fn resolution(&self) -> &ImmutableSourceResolution {
            &self.resolution
        }

        pub const fn materialization(&self) -> &PackageSourceMaterialization {
            &self.materialization
        }

        pub fn snapshot_root(&self) -> &std::path::Path {
            &self.snapshot_root
        }

        pub const fn navigation(&self) -> &PackageSourceNavigation {
            &self.navigation
        }

        pub const fn selection_evidence(&self) -> &PackageSourceSelectionEvidence {
            &self.selection_evidence
        }

        pub fn source_limits(&self) -> LocalSourceLimits {
            self.source_limits
        }

        /// Authored requests for one purpose scope, in requester-local order.
        pub fn dependency_requests(
            &self,
            purpose: DependencyPurpose,
        ) -> &[DependencySourceRequest] {
            self.dependency_projections.requests(purpose)
        }



        pub const fn dependency_projections(&self) -> &DependencyProjections {
            &self.dependency_projections
        }
    };
}
pub(crate) use package_source_surface_accessors;
