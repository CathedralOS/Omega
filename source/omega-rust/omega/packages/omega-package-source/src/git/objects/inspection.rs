//! Whole-tree and selective inspection over a retained Git repository.

use std::ffi::OsStr;

use crate::error::SourceResolveError;
use crate::git::cache::identity::cache_invalid;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::limits::LocalSourceLimits;

use super::GitTreeEntry;
use super::authentication::authenticate_git_tree_graph;
use super::batch::read_git_blobs_batch;
use super::graph::AuthenticatedGitTreeGraph;
use super::identity::is_object_id;
use super::projection::{
    AuthenticatedGitTreeProjection, GitTreeProjectionPlan, GitTreeProjectionRequest,
};
use super::tree::{parse_git_tree_entries, parse_git_tree_graph_entries};

pub(crate) fn inspect_git_tree(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    limits: LocalSourceLimits,
) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
    validate_tree_oid(repository, tree)?;
    let listing = list_tree(executor, repository, tree)?;
    let mut entries = parse_git_tree_entries(&listing, repository.path(), limits)?;
    authenticate_git_tree_graph(tree, &entries)?;
    read_git_blobs_batch(executor, repository, &mut entries, limits)?;
    Ok(entries)
}

pub(crate) fn inspect_git_tree_projection(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    request: &GitTreeProjectionRequest,
    limits: LocalSourceLimits,
) -> Result<AuthenticatedGitTreeProjection, SourceResolveError> {
    validate_tree_oid(repository, tree)?;
    let listing = list_tree(executor, repository, tree)?;
    let entries = parse_git_tree_graph_entries(&listing, repository.path())?;
    let graph = AuthenticatedGitTreeGraph::authenticate(tree, entries)?;
    let plan = GitTreeProjectionPlan::from_graph(&graph, request, limits)?;
    plan.open_and_authenticate(executor, repository, limits)
}

fn validate_tree_oid(
    repository: &VerifiedGitRepository,
    tree: &str,
) -> Result<(), SourceResolveError> {
    if !is_object_id(tree) {
        return Err(cache_invalid(
            repository.path(),
            "Git returned an invalid tree object ID",
        ));
    }
    Ok(())
}

fn list_tree(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
) -> Result<Vec<u8>, SourceResolveError> {
    repository.run_git_bytes_stdout(
        executor,
        [
            OsStr::new("ls-tree"),
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-t"),
            OsStr::new("-l"),
            OsStr::new("-z"),
            OsStr::new(tree),
        ],
    )
}
