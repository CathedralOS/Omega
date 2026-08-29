//! Package declarations and keys recovered from immutable source snapshots.
//!
//! The entry points are grouped by source lineage. Shared declaration
//! projection, errors, and resolved custody live behind this narrow facade.

mod custody;
mod error;
mod git;
mod local;
mod projection;
mod resolved;
mod workspace;

pub use custody::PackageSourceCustody;
pub use error::ResolvePackageSourceError;
#[cfg(test)]
pub(crate) use git::resolve_git_package_source;
pub(crate) use git::resolve_git_package_source_in_lane;
pub use git::resolve_git_package_source_with_storage;
#[cfg(test)]
pub(crate) use local::resolve_external_local_package_source;
pub(crate) use local::{
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
};
pub use local::{
    resolve_external_local_package_source_with_storage,
    resolve_external_local_project_source_with_storage,
};
pub use resolved::ResolvedPackageSource;
#[cfg(test)]
pub(crate) use workspace::resolve_workspace_member_package_source;
pub(crate) use workspace::resolve_workspace_member_package_source_in_lane;
pub use workspace::resolve_workspace_member_package_source_with_storage;

#[cfg(test)]
mod tests;
