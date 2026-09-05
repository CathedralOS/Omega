//! Resolve a validated Git request into final immutable source custody.
//!
//! The lifecycle is visible in this directory: [`acquisition`] opens or creates
//! the retained cache entry, [`repository`] authenticates the exact commit and
//! tree, a materializer publishes either the whole source or one
//! [`workspace_member`], and [`issuance`] revalidates custody before returning.

use crate::error::SourceResolveError;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
use crate::storage::{RetainedStorageLane, SourceResolverStorage};
use cap_std::fs::Dir as CapabilityDirectory;
use std::path::{Path, PathBuf};

use super::request::GitSourceRequest;
use super::workspace::GitWorkspaceProjectionError;

mod acquisition;
mod exact_revision;
mod issuance;
mod materialization;
mod network;
mod pinned_source;
mod recorded_objects;
mod repository;
mod selection;
mod workspace_member;

use acquisition::resolve_git_source_from_retained_cache_with;
use materialization::materialize_whole_git_source;

pub use pinned_source::{
    resolve_git_source_from_pin_in_lane, resolve_git_source_from_pin_in_lane_with_primary_git,
};

pub use exact_revision::{
    GitExactRevisionAcquisition, resolve_git_source_at_revision_in_lane,
    resolve_git_source_at_revision_in_lane_with_primary_git,
};

pub use workspace_member::{
    resolve_git_workspace_member_from_pin_in_lanes,
    resolve_git_workspace_member_from_pin_in_lanes_with_primary_git,
    resolve_git_workspace_member_in_lanes, resolve_git_workspace_member_in_lanes_with_primary_git,
    resolve_git_workspace_member_with_primary_git, resolve_git_workspace_member_with_storage,
};

#[cfg(test)]
mod tests;

pub fn resolve_git_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let package_controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
    resolve_git_source_in_lane_with_selected_roots(
        lane.primary_git()?,
        &package_controlled_roots,
        request,
        None,
        lane,
        limits,
    )
}

pub fn resolve_git_source_in_lane_with_primary_git(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let package_controlled_roots = resolver_package_controlled_roots(&[lane.path()])?;
    resolve_git_source_in_lane_with_selected_roots(
        primary_git,
        &package_controlled_roots,
        request,
        None,
        lane,
        limits,
    )
}

fn resolve_git_source_in_lane_with_selected_roots(
    primary_git: &PrimaryGitSelection,
    package_controlled_roots: &[PathBuf],
    request: &GitSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    lane.verify_path_identity()?;
    let result = resolve_git_source_from_retained_cache(
        primary_git,
        package_controlled_roots,
        request,
        pin,
        lane.path(),
        lane.directory(),
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

pub fn resolve_git_source_with_primary_git(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    storage.verify_path_identity()?;
    let result = resolve_git_source_in_lane_with_primary_git(
        primary_git,
        request,
        storage.git_sources(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}

fn resolve_git_source_from_retained_cache(
    primary_git: &PrimaryGitSelection,
    package_controlled_roots: &[PathBuf],
    request: &GitSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    match resolve_git_source_from_retained_cache_with(
        primary_git,
        package_controlled_roots,
        request,
        cache_dir,
        cache_directory,
        limits,
        pin,
        materialize_whole_git_source,
    ) {
        Ok((source, ())) => Ok(source),
        Err(GitWorkspaceProjectionError::Source(error)) => Err(error),
        Err(GitWorkspaceProjectionError::Planner(never)) => match never {},
    }
}
