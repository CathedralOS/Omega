//! Resolve a validated Git request into final immutable source custody.
//!
//! The lifecycle is visible in this directory: [`acquisition`] opens or creates
//! the retained cache entry, [`repository`] authenticates the exact commit and
//! tree, a materializer publishes either the whole source or one
//! [`workspace_member`], and [`issuance`] revalidates custody before returning.

use crate::SourceResolver;
use crate::error::SourceResolveError;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;

use super::request::GitSourceRequest;

mod acquisition;
mod exact_revision;
mod issuance;
mod materialization;
mod network;
mod pinned_source;
mod recorded_objects;
mod repository;
mod selection;
mod tree_session;
mod workspace_member;
pub use tree_session::{GitWorkspaceCatalog, GitWorkspaceSession};

use acquisition::resolve_whole_git_source_from_retained_lanes;
use selection::GitRevisionSelection;

pub use pinned_source::resolve_git_source_from_pin_in_lane;

pub use exact_revision::{GitExactRevisionAcquisition, resolve_git_source_at_revision_in_lane};

pub use workspace_member::{
    resolve_git_workspace_member_at_revision_in_lanes,
    resolve_git_workspace_member_from_pin_in_lanes,
};

#[cfg(test)]
pub(crate) use workspace_member::resolve_git_workspace_member;

#[cfg(test)]
mod tests;

/// Resolve one Git source inside a retained lane with the selected primary Git.
/// Callers pass the session selection from `resolver.primary_git()?`.
pub fn resolve_git_source_in_lane(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    resolve_whole_git_source_from_retained_lanes(
        primary_git,
        &[lane],
        request,
        limits,
        GitRevisionSelection::Ordinary(None),
    )
}

pub(crate) fn resolve_git_source(
    request: &GitSourceRequest,
    resolver: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    resolver.verify_path_identity()?;
    let result = resolve_git_source_in_lane(
        resolver.primary_git()?,
        request,
        resolver.git_sources(),
        limits,
    );
    resolver.verify_path_identity()?;
    result
}
