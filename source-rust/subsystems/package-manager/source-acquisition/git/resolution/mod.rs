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
`selection` and `exact_objects` isolate when transport is permitted so an
offline or pinned lookup never drifts to a new ref or mistakes a missing
historical object for cache corruption.
*/

use super::request::GitSourceRequest;
#[cfg(test)]
use crate::SourceResolver;
use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::GitExecutor;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::objects::inspect_git_tree;
use crate::git::snapshot::resolve_git_snapshot;
use crate::git::workspace::{GitWorkspaceProjectionCustody, GitWorkspaceProjectionError};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::ResolvedGitSource;
use crate::storage::RetainedStorageLane;
use crate::tree::ResolvedLocalSource;
use acquisition::resolve_whole_git_source_from_retained_lanes;
pub use exact_revision::{GitExactRevisionAcquisition, resolve_git_source_at_revision_in_lane};
pub use pinned_source::resolve_git_source_from_pin_in_lane;
use selection::GitRevisionSelection;
use std::ffi::OsString;
use std::path::PathBuf;
pub use tree_session::{GitWorkspaceCatalog, GitWorkspaceSession};
pub use workspace_member::{
    resolve_git_workspace_member_at_revision_in_lanes,
    resolve_git_workspace_member_from_pin_in_lanes,
};

mod acquisition;
mod exact_objects;
mod exact_revision;
mod issuance;
mod pinned_source;
mod repository;
mod selection;
mod tree_session;
mod workspace_member;

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

// Internal materialization result (`GitMaterializedSource`) shared by
// whole-repository (`materialize_whole_git_source`) and workspace-member
// materializers.

struct GitMaterializedSource<Evidence> {
    pub(super) materialized_tree: String,
    pub(super) snapshot_root: PathBuf,
    pub(super) local: ResolvedLocalSource,
    pub(super) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    pub(super) evidence: Evidence,
}

fn materialize_whole_git_source(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<GitMaterializedSource<()>, GitWorkspaceProjectionError<std::convert::Infallible>> {
    let entries = inspect_git_tree(executor, repository, tree, limits)?;
    let (snapshot_root, local) = resolve_git_snapshot(executor, repository, tree, entries, limits)?;
    Ok(GitMaterializedSource {
        materialized_tree: tree.to_owned(),
        snapshot_root,
        local,
        workspace_projection: None,
        evidence: (),
    })
}

// Compiler-owned bounded `git fetch` argument construction
// (`--depth=1 --no-tags --no-recurse-submodules --filter=blob:limit=<max_bytes + 1>`).

pub(crate) fn bounded_git_fetch_arguments(
    fetch_locator: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Vec<OsString> {
    let first_inadmissible_blob_size = limits
        .max_bytes
        .checked_add(1)
        .expect("compiler-owned Git source byte ceiling leaves room for one sentinel byte");
    vec![
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--depth=1"),
        OsString::from("--no-tags"),
        OsString::from("--no-recurse-submodules"),
        OsString::from(format!(
            "--filter=blob:limit={first_inadmissible_blob_size}"
        )),
        OsString::from("--"),
        OsString::from(fetch_locator),
        OsString::from(requested_rev),
    ]
}
