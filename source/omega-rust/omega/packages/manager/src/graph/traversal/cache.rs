use crate::package::{
    ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
    resolve_git_package_source_in_lane, resolve_workspace_member_package_source_in_lane,
};
use crate::source::RetainedStorageLane;
use crate::source::identity::{ExternalSourceContext, SourceLineage, WorkspaceMemberPath};
use crate::source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot,
};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum SourceCacheLane<'a> {
    Retained(&'a RetainedStorageLane),
}

pub(super) fn resolve_git_from_cache(
    request: &GitSourceRequest,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_git_package_source_in_lane(request, lane, limits)
        }
    }
}

pub(super) fn resolve_external_local_package_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_package_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

pub(super) fn resolve_external_local_project_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_project_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

pub(super) fn resolve_workspace_member_from_cache(
    workspace_root_source: &SourceLineage,
    member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => resolve_workspace_member_package_source_in_lane(
            workspace_root_source,
            member_path,
            live_workspace_root,
            lane,
            limits,
        ),
    }
}
