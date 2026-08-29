//! Internal result shared by whole-repository and member materializers.

use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::inspect_git_tree;
use crate::git::snapshot::materialization::resolve_git_snapshot;
use crate::git::workspace::{GitWorkspaceProjectionCustody, GitWorkspaceProjectionError};
use crate::limits::LocalSourceLimits;
use crate::local::model::ResolvedLocalSource;
use std::path::PathBuf;

pub(super) struct GitMaterializedSource<Evidence> {
    pub(super) materialized_tree: String,
    pub(super) snapshot_root: PathBuf,
    pub(super) local: ResolvedLocalSource,
    pub(super) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    pub(super) evidence: Evidence,
}

pub(super) fn materialize_whole_git_source(
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
