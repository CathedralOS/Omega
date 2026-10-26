//! Explicit local, workspace, and Git source policies for closure traversal.
//!
//! Start here to discover the workflow: each root-source kind owns its entry
//! path, while dependency resolution and retained-source access are shared
//! implementation details.

mod cache;
mod dependencies;
mod errors;
mod external_local;
mod git_pins;
mod locked;

pub(crate) use errors::{ResolveDependencySourceError, ResolveExternalLocalPackageClosureError};
pub(crate) use external_local::resolve_external_local_project_closure;
pub(crate) use git_pins::{GitDependencyPins, GitResolutionOptions};
pub(crate) use locked::{
    admit_locked_source_request, mutable_local_graph_matches, resolve_locked_local_project_closure,
};
