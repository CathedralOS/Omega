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
    ResolveDependencySourceError, ResolveExternalLocalPackageClosureError,
    ResolveGitPackageClosureError, ResolveWorkspacePackageClosureError,
};
pub use external_local::{
    resolve_external_local_package_closure_with_storage,
    resolve_external_local_project_closure_with_options,
    resolve_external_local_project_closure_with_storage,
    resolve_staged_external_local_project_closure_with_git_pins,
    resolve_staged_external_local_project_closure_with_options,
    resolve_staged_external_local_project_closure_with_storage,
};
pub use git::{
    resolve_git_package_closure_with_storage, resolve_git_project_closure_with_storage,
    resolve_selected_git_package_closure_with_storage,
    resolve_selected_git_project_closure_with_storage,
};
pub use git_pins::{GitDependencyPins, GitDependencyPinsError, GitResolutionOptions};
pub use locked::{
    ResolveLockedPackageClosureError, resolve_locked_local_project_closure_with_storage,
    resolve_locked_package_source_closure_with_storage,
};
pub use workspace::{
    resolve_workspace_package_closure_in_context_with_storage,
    resolve_workspace_package_closure_with_storage,
    resolve_workspace_project_closure_in_context_with_storage,
    resolve_workspace_project_closure_with_storage,
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
use super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
};
#[cfg(test)]
use package_source::{ExternalSourceContext, SourceLineage, SourceRelativePath};
#[cfg(test)]
use package_source::{GitSourceRequest, LocalSourceLimits, SourceResolverStorage};
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;
