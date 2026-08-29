//! Package declarations and keys recovered from immutable source snapshots.
//!
//! The entry points are grouped by source lineage. Shared declaration
//! projection, errors, and resolved custody live behind this narrow facade.

mod error;
mod git;
mod local;
mod projection;
mod resolved;
mod workspace;

pub use error::ResolvePackageSourceError;
pub use git::resolve_git_package_source;
pub use local::{resolve_external_local_package_source, resolve_external_local_project_source};
pub use resolved::ResolvedPackageSource;
pub use workspace::resolve_workspace_member_package_source;

#[cfg(test)]
mod tests;
