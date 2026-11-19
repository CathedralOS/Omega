/*
Git source resolution lifecycle from validated request through authenticated
immutable snapshot issuance.

We coordinate Git acquisition across four stages: `acquisition` opens or creates
the retained bare-repository cache entry under an entry lock and verifies lane
path identities around the entire resolution; `repository` selects or fetches
the target commit and authenticates the commit and root tree objects; a
materializer (`materialization` for whole repositories, `workspace_member` for
projected workspace packages, or `tree_session` for staged multi-step catalog
discovery) authenticates and publishes the selected snapshot; and `issuance`
re-verifies snapshot bytes and retained storage custody before returning
`ResolvedGitSource`.
*/

/*
Why selection intent is separated from authenticated source custody:

Callers may resolve a floating selector (`resolve_git_source_in_lane`), reuse an
operation-local acquisition pin (`resolve_git_source_from_pin_in_lane`), or
reconstruct an exact recorded commit and tree from a lockfile
(`resolve_git_source_at_revision_in_lane` with `Offline` or `AllowFetch`).
`selection` and `recorded_objects` isolate when transport is permitted so an
offline or pinned lookup never drifts to a new ref or mistakes a missing
historical object for cache corruption.
*/

#[cfg(test)]
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

#[cfg(test)]
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
