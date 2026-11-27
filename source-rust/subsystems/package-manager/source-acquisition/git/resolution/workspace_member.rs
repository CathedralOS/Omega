// Snapshot publication for one declared member of an authenticated Git
// workspace (`materialize_member`), after `GitWorkspaceSession` has read the
// root and requested member declarations (`declarations`).
//
// Both the bare-repository `git_lane` and the `member_lane` prove their path
// identities around the resolution. Because identical Git member subtrees can
// belong to distinct package lineages that require disjoint physical source
// roots, `materialize_member` scopes the member publication beneath a
// repository-specific child directory (`member_lane.retain_child(repository_namespace)`)
// rather than sharing a content-only member namespace.

use super::GitMaterializedSource;
use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::GitExecutor;
use crate::git::objects::{AuthenticatedGitTreeInspection, GitTreeProjectionRequest};
use crate::git::snapshot::publish_git_member_snapshot;
use crate::git::workspace::{GitWorkspaceDeclaration, GitWorkspaceProjectionCustody};
use crate::limits::LocalSourceLimits;
use crate::storage::RetainedStorageLane;

pub(super) mod declarations;

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_member(
    inspection: &AuthenticatedGitTreeInspection,
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    root_declaration: GitWorkspaceDeclaration,
    member_declarations: Vec<GitWorkspaceDeclaration>,
    selected_member_path: crate::SourceRelativePath,
) -> Result<GitMaterializedSource, SourceResolveError> {
    let projection = inspection.project(
        executor,
        repository,
        &GitTreeProjectionRequest::new(
            std::iter::empty(),
            selected_member_path.as_str().as_bytes().to_vec(),
        ),
        limits,
    )?;
    let member = projection.into_member();
    let materialized_tree = member.tree_oid().to_owned();
    let workspace_projection = GitWorkspaceProjectionCustody::new(
        root_declaration,
        member_declarations,
        selected_member_path,
        materialized_tree.clone(),
    );
    let repository_namespace = repository
        .entry_name
        .to_str()
        .expect("compiler-generated Git cache names are ASCII");
    let scoped_member_lane = member_lane.retain_child(repository_namespace)?;
    let (snapshot_root, local) = publish_git_member_snapshot(
        executor,
        &scoped_member_lane,
        &materialized_tree,
        member.into_entries(),
        limits,
    )?;

    Ok(GitMaterializedSource {
        materialized_tree,
        snapshot_root,
        local,
        workspace_projection: Some(workspace_projection),
    })
}
