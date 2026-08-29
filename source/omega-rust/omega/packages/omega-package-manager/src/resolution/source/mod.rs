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
pub use error::ResolvePackageSourceError;
#[cfg(test)]
pub(crate) use git::resolve_git_package_source;
pub(crate) use git::resolve_selected_git_package_source_from_pin_in_lanes;
pub(crate) use git::resolve_selected_git_project_source_from_pin_in_lanes;
pub(crate) use git::workspace::{GitWorkspaceSelectionDeclarations, GitWorkspaceSelectionEvidence};
pub use git::{
    GitPackageSourceRequest, resolve_git_package_source_with_storage,
    resolve_selected_git_package_source_with_storage,
    resolve_selected_git_project_source_with_storage,
};
#[cfg(test)]
pub(crate) use local::resolve_external_local_package_source;
pub(crate) use local::{
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
};
pub use local::{
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage,
};
pub use materialization::PackageSourceMaterialization;
pub use navigation::PackageSourceNavigation;
pub use resolved::ResolvedPackageSource;
pub use selection::{PackageSourceSelectionEvidence, PackageSourceSelectionEvidenceError};
#[cfg(test)]
pub(crate) use workspace::resolve_workspace_member_package_source;
pub(crate) use workspace::resolve_workspace_member_package_source_in_lane;
pub(crate) use workspace::resolve_workspace_member_project_source_in_lane;
pub use workspace::{
    resolve_workspace_member_package_source_with_storage,
    resolve_workspace_member_project_source_with_storage,
};

#[cfg(test)]
mod tests;
