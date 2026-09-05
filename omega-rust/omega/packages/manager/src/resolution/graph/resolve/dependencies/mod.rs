//! Resolve declared dependency rows after a root source enters custody.

mod context;
mod external_local;
mod git;
mod path;

pub(super) use context::WorkspaceContext;
pub(super) use git::register_git_repository;
pub(super) use path::{normalize_member_path, workspace_path_escapes};

use super::super::reconcile::{
    PackageRootSourceRequest, PackageSourceClosureLimits, PackageSourceClosureResolutionError,
    ResolvedPackageSourceClosure, resolve_package_source_closure_with_limits,
};
use super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::errors::ResolveDependencySourceError;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::source::PackageSourceCustody;
use package_source::{ExternalSourceContext, LocalSourceLimits, WorkspaceLineageIdentity};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_registered_package_closure(
    root_request: PackageRootSourceRequest,
    root: PackageSourceCustody,
    closure_limits: PackageSourceClosureLimits,
    workspace_cache: SourceCacheLane<'_>,
    git_cache: SourceCacheLane<'_>,
    external_local_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    external_roots: &mut BTreeMap<PackageKey, PathBuf>,
    external_context: Option<&ExternalSourceContext>,
    git_acquisitions: &mut GitAcquisitionCache,
) -> Result<
    ResolvedPackageSourceClosure,
    PackageSourceClosureResolutionError<ResolveDependencySourceError>,
> {
    resolve_package_source_closure_with_limits(
        root_request,
        root,
        closure_limits,
        |requester, request| match request {
            DependencySourceRequest::Git {
                repository,
                revision,
                selection,
                ..
            } => git::resolve_git_dependency(
                repository,
                revision,
                selection,
                workspaces,
                git_acquisitions,
                git_cache,
                workspace_cache,
                source_limits,
            ),
            DependencySourceRequest::Path { location, .. } => path::resolve_path_dependency(
                requester,
                location,
                workspaces,
                external_roots,
                external_context,
                workspace_cache,
                git_cache,
                external_local_cache,
                source_limits,
                git_acquisitions,
            ),
        },
    )
}
