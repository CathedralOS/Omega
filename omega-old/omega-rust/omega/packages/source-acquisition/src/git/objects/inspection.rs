// Whole-tree and selective graph inspection over a retained Git repository.
//
// `AuthenticatedGitTreeInspection` holds a payload-free authenticated tree
// graph (`AuthenticatedGitTreeGraph`) across declaration discovery and workspace
// member selection so callers can inspect root declarations, derive member
// declaration paths, and batch-read only the selected payload closure without
// repeating graph authentication. `open_catalog_inventory` filters out inert
// foreign `Gitlink` edges while authenticating all supported repository blobs.

use std::ffi::OsStr;

use crate::error::SourceResolveError;
use crate::error::cache_invalid;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::limits::LocalSourceLimits;

use super::GitTreeEntry;
use super::authentication::{authenticate_git_tree_graph, authenticate_git_tree_payloads};
use super::batch::read_git_blobs_batch;
use super::graph::AuthenticatedGitTreeGraph;
use super::identity::is_object_id;
use super::projection::AuthenticatedGitTreeProjection;
use super::projection::{GitTreeProjectionPlan, GitTreeProjectionRequest, select_regular_files};
use super::tree::{parse_git_tree_entries, parse_git_tree_graph_entries};

#[derive(Debug)]
pub(crate) struct AuthenticatedGitTreeInspection {
    graph: AuthenticatedGitTreeGraph,
}

impl AuthenticatedGitTreeInspection {
    pub(crate) fn open_catalog_inventory(
        &self,
        executor: &GitExecutor,
        repository: &VerifiedGitRepository,
        limits: LocalSourceLimits,
    ) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
        let mut entries = self
            .graph
            .entries()
            .iter()
            .filter(|entry| !matches!(entry.kind, super::GitTreeEntryKind::Gitlink))
            .cloned()
            .collect::<Vec<_>>();
        read_git_blobs_batch(executor, repository, &mut entries, limits)?;
        authenticate_git_tree_payloads(self.graph.root_tree_oid(), &entries)?;
        Ok(entries)
    }

    pub(crate) fn root_tree_oid(&self) -> &str {
        self.graph.root_tree_oid()
    }

    pub(crate) fn open_regular_files(
        &self,
        executor: &GitExecutor,
        repository: &VerifiedGitRepository,
        exact_paths: impl IntoIterator<Item = Vec<u8>>,
        limits: LocalSourceLimits,
    ) -> Result<Vec<GitTreeEntry>, SourceResolveError> {
        let mut entries =
            select_regular_files(&self.graph, exact_paths.into_iter().collect(), limits)?;
        read_git_blobs_batch(executor, repository, &mut entries, limits)?;
        authenticate_git_tree_payloads(self.graph.root_tree_oid(), &entries)?;
        Ok(entries)
    }

    pub(crate) fn project(
        &self,
        executor: &GitExecutor,
        repository: &VerifiedGitRepository,
        request: &GitTreeProjectionRequest,
        limits: LocalSourceLimits,
    ) -> Result<AuthenticatedGitTreeProjection, SourceResolveError> {
        GitTreeProjectionPlan::from_graph(&self.graph, request, limits)?
            .open_and_authenticate(executor, repository, limits)
    }
}

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

#[cfg(test)]
pub(crate) fn inspect_git_tree_projection(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    request: &GitTreeProjectionRequest,
    limits: LocalSourceLimits,
) -> Result<AuthenticatedGitTreeProjection, SourceResolveError> {
    inspect_git_tree_graph(executor, repository, tree)?
        .project(executor, repository, request, limits)
}

pub(crate) fn inspect_git_tree_graph(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
) -> Result<AuthenticatedGitTreeInspection, SourceResolveError> {
    validate_tree_oid(repository, tree)?;
    let listing = list_tree(executor, repository, tree)?;
    let entries = parse_git_tree_graph_entries(&listing, repository.path())?;
    Ok(AuthenticatedGitTreeInspection {
        graph: AuthenticatedGitTreeGraph::authenticate(tree, entries)?,
    })
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
