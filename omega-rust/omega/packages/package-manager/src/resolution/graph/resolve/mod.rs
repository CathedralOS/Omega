//! Explicit local, workspace, and Git source policies for closure traversal.
//!
//! Start here to discover the workflow: each root-source kind owns its entry
//! path, while dependency resolution and retained-source access are shared
//! implementation details.

mod cache;
mod dependencies;
mod errors;
mod external_local;
mod git;
mod git_pins;
mod locked;
mod workspace;

pub use errors::{
};
pub(crate) use errors::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveWorkspacePackageClosureError};
pub use external_local::{
};
pub(crate) use external_local::{
    resolve_external_local_project_closure,
    resolve_staged_external_local_project_closure};
pub(crate) use git_pins::{GitDependencyPins, GitResolutionOptions};
pub use locked::{
};
pub(crate) use locked::{
    resolve_locked_local_project_closure,
    resolve_locked_package_source_closure};
pub use workspace::{
};
pub(crate) use workspace::{
    resolve_workspace_project_closure};
