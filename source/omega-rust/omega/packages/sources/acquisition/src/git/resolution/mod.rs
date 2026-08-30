//! Resolve a validated Git request into final immutable source custody.
//!
//! The lifecycle is visible in this directory: [`acquisition`] opens or creates
//! the retained cache entry, [`repository`] authenticates the exact commit and
//! tree, a materializer publishes either the whole source or one
//! [`workspace_member`], and [`issuance`] revalidates custody before returning.

use crate::error::SourceResolveError;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::{RetainedStorageLane, SourceResolverStorage};
use cap_std::fs::Dir as CapabilityDirectory;
use std::path::Path;

use super::request::GitSourceRequest;
use super::workspace::GitWorkspaceProjectionError;

mod acquisition;
mod issuance;
mod materialization;
mod network;
mod repository;
mod workspace_member;

use acquisition::resolve_git_source_from_retained_cache_with;
use materialization::materialize_whole_git_source;

pub use workspace_member::{
    resolve_git_workspace_member_from_pin_in_lanes, resolve_git_workspace_member_in_lanes,
    resolve_git_workspace_member_with_storage,
};

#[cfg(test)]
mod tests;

pub fn resolve_git_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    lane.verify_path_identity()?;
    let result = resolve_git_source_from_retained_cache(
        request,
        lane.path(),
        lane.directory(),
        lane.primary_git_path()?,
        limits.compiler_bounded(),
    );
    lane.verify_path_identity()?;
    result
}

pub fn resolve_git_source_with_storage(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    storage.verify_path_identity()?;
    let result = resolve_git_source_in_lane(request, storage.git_sources(), limits);
    storage.verify_path_identity()?;
    result
}

fn resolve_git_source_from_retained_cache(
    request: &GitSourceRequest,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    primary_git: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    match resolve_git_source_from_retained_cache_with(
        request,
        cache_dir,
        cache_directory,
        primary_git,
        limits,
        None,
        materialize_whole_git_source,
    ) {
        Ok((source, ())) => Ok(source),
        Err(GitWorkspaceProjectionError::Source(error)) => Err(error),
        Err(GitWorkspaceProjectionError::Planner(never)) => match never {},
    }
}
