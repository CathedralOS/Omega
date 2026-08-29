//! Explicit local, workspace, and Git source policies for closure traversal.
//!
//! Start here to discover the workflow: each root-source kind owns its entry
//! path, while dependency resolution and retained-source access are shared
//! implementation details.

mod cache;
mod dependency_resolution;
mod errors;
mod external_local;
mod git;
mod workspace;

pub use errors::{
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
};
pub use external_local::{
    resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_storage,
};
pub use git::resolve_git_package_closure_with_storage;
pub use workspace::{
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
};

#[cfg(test)]
pub(crate) use external_local::resolve_external_local_package_closure;
#[cfg(test)]
pub(crate) use git::{git_root_request_matches, resolve_git_package_closure};
#[cfg(test)]
pub(crate) use workspace::{
    resolve_workspace_package_closure, resolve_workspace_package_closure_in_context,
};

#[cfg(test)]
use super::reconciliation::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
#[cfg(test)]
use crate::source::identity::{ExternalSourceContext, SourceLineage, WorkspaceMemberPath};
#[cfg(test)]
use crate::source::{GitSourceRequest, LocalSourceLimits, SourceResolverStorage};
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;
