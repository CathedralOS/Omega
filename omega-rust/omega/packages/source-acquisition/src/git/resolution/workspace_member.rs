//! Select and publish one declared member from an authenticated Git workspace.

use crate::SourceResolver;
use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::objects::{
    AuthenticatedGitTreeInspection, GitTreeProjectionRequest, inspect_git_tree_graph,
};
use crate::git::request::GitSourceRequest;
use crate::git::snapshot::publish_git_member_snapshot;
use crate::git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionCustody,
    GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult,
};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::GitAcquisitionPin;
use crate::storage::RetainedStorageLane;

use super::acquisition::resolve_git_source_from_retained_cache;
use super::materialization::GitMaterializedSource;
use super::selection::GitRevisionSelection;

pub(super) mod declarations;
mod recorded;
pub use recorded::resolve_git_workspace_member_at_revision_in_lanes;

pub(crate) fn resolve_git_workspace_member<Planner>(
    request: &GitSourceRequest,
    resolver: &SourceResolver,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    resolver.verify_path_identity()?;
    let result = resolve_git_workspace_member_from_pin_in_lanes(
        resolver.primary_git()?,
        request,
        None,
        resolver.git_sources(),
        resolver.workspace_members(),
        limits.compiler_bounded(),
        declaration_limits,
        planner,
    );
    resolver.verify_path_identity()?;
    result
}

pub fn resolve_git_workspace_member_from_pin_in_lanes<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    pin: Option<&GitAcquisitionPin>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    resolve_git_workspace_member_at_selection(
        primary_git,
        request,
        GitRevisionSelection::Ordinary(pin),
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

/// One member selection inside the retained lanes the resolution is issued in:
/// the cache lane and the member lane both prove their path identity around it.
#[allow(clippy::too_many_arguments)]
fn resolve_git_workspace_member_at_selection<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    selection: GitRevisionSelection<'_>,
    git_lane: &RetainedStorageLane,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<
    GitWorkspaceProjectionResult<Planner::Evidence>,
    GitWorkspaceProjectionError<Planner::Error>,
>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    resolve_git_source_from_retained_cache(
        primary_git,
        &[git_lane, member_lane],
        request,
        limits,
        selection,
        |executor, repository, tree, limits| {
            project_git_workspace_member(
                executor,
                repository,
                tree,
                member_lane,
                limits,
                declaration_limits,
                planner,
            )
        },
    )
    .map(|(source, evidence)| GitWorkspaceProjectionResult::new(source, evidence))
}

fn project_git_workspace_member<Planner>(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    tree: &str,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
    planner: &mut Planner,
) -> Result<GitMaterializedSource<Planner::Evidence>, GitWorkspaceProjectionError<Planner::Error>>
where
    Planner: GitWorkspaceProjectionPlanner,
{
    let inspection = inspect_git_tree_graph(executor, repository, tree)?;
    debug_assert_eq!(inspection.root_tree_oid(), tree);
    let root_declaration = declarations::open_root(
        &inspection,
        executor,
        repository,
        limits,
        declaration_limits,
    )?;
    let member_paths = planner
        .discover_members(&root_declaration)
        .map_err(GitWorkspaceProjectionError::Planner)?;
    let member_declarations = declarations::open_members(
        &inspection,
        executor,
        repository,
        &root_declaration,
        &member_paths,
        limits,
        declaration_limits,
    )?;
    let selection = planner
        .select_member(&root_declaration, &member_declarations)
        .map_err(GitWorkspaceProjectionError::Planner)?;
    let (selected_member_path, evidence) = selection.into_parts();
    if !member_paths.contains(&selected_member_path) {
        return Err(SourceResolveError::GitTreeInvalid {
            path: selected_member_path.as_str().as_bytes().to_vec(),
            message: "workspace planner selected a member absent from its discovered set"
                .to_owned(),
        }
        .into());
    }

    materialize_member(
        &inspection,
        executor,
        repository,
        member_lane,
        limits,
        root_declaration,
        member_declarations,
        selected_member_path,
        evidence,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_member<Evidence, PlannerError>(
    inspection: &AuthenticatedGitTreeInspection,
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    member_lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    root_declaration: GitWorkspaceDeclaration,
    member_declarations: Vec<GitWorkspaceDeclaration>,
    selected_member_path: crate::SourceRelativePath,
    evidence: Evidence,
) -> Result<GitMaterializedSource<Evidence>, GitWorkspaceProjectionError<PlannerError>> {
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
    // Equal Git trees can belong to distinct package lineages. The compiler
    // requires disjoint physical source roots for those distinct identities;
    // content-only member storage would merge their routing custody.
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
        evidence,
    })
}
