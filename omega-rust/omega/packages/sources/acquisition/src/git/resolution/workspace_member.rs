//! Select and publish one declared member from an authenticated Git workspace.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::git::objects::{
    GitTreeEntry, GitTreeEntryKind, GitTreeProjectionRequest, inspect_git_tree_graph,
};
use crate::git::request::GitSourceRequest;
use crate::git::snapshot::publish_git_member_snapshot;
use crate::git::workspace::{
    GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits, GitWorkspaceProjectionCustody,
    GitWorkspaceProjectionError, GitWorkspaceProjectionPlanner, GitWorkspaceProjectionResult,
};
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::GitAcquisitionPin;
use crate::storage::{RetainedStorageLane, SourceResolverStorage};
use std::collections::BTreeSet;
use std::path::PathBuf;

use super::acquisition::resolve_git_source_from_retained_cache_with_selection;
use super::materialization::GitMaterializedSource;
use super::selection::GitRevisionSelection;

mod recorded;
pub use recorded::{
    resolve_git_workspace_member_at_revision_in_lanes,
    resolve_git_workspace_member_at_revision_in_lanes_with_primary_git,
};

pub fn resolve_git_workspace_member_with_storage<Planner>(
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
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
    storage.verify_path_identity()?;
    let result = resolve_git_workspace_member_in_lanes(
        request,
        storage.git_sources(),
        storage.workspace_members(),
        limits.compiler_bounded(),
        declaration_limits,
        planner,
    );
    storage.verify_path_identity()?;
    result
}

pub fn resolve_git_workspace_member_with_primary_git<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
    storage: &SourceResolverStorage,
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
    storage.verify_path_identity()?;
    let result = resolve_git_workspace_member_in_lanes_with_primary_git(
        primary_git,
        request,
        storage.git_sources(),
        storage.workspace_members(),
        limits.compiler_bounded(),
        declaration_limits,
        planner,
    );
    storage.verify_path_identity()?;
    result
}

pub fn resolve_git_workspace_member_in_lanes<Planner>(
    request: &GitSourceRequest,
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
    resolve_git_workspace_member_from_pin_in_lanes(
        request,
        None,
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

pub fn resolve_git_workspace_member_in_lanes_with_primary_git<Planner>(
    primary_git: &PrimaryGitSelection,
    request: &GitSourceRequest,
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
    resolve_git_workspace_member_from_pin_in_lanes_with_primary_git(
        primary_git,
        request,
        None,
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

pub fn resolve_git_workspace_member_from_pin_in_lanes<Planner>(
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
    let package_controlled_roots =
        resolver_package_controlled_roots(&[git_lane.path(), member_lane.path()])?;
    resolve_git_workspace_member_with_selected_roots(
        git_lane
            .primary_git()
            .map_err(GitWorkspaceProjectionError::Source)?,
        &package_controlled_roots,
        request,
        GitRevisionSelection::Ordinary(pin),
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

pub fn resolve_git_workspace_member_from_pin_in_lanes_with_primary_git<Planner>(
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
    let package_controlled_roots =
        resolver_package_controlled_roots(&[git_lane.path(), member_lane.path()])?;
    resolve_git_workspace_member_with_selected_roots(
        primary_git,
        &package_controlled_roots,
        request,
        GitRevisionSelection::Ordinary(pin),
        git_lane,
        member_lane,
        limits,
        declaration_limits,
        planner,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_git_workspace_member_with_selected_roots<Planner>(
    primary_git: &PrimaryGitSelection,
    package_controlled_roots: &[PathBuf],
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
    git_lane.verify_path_identity()?;
    member_lane.verify_path_identity()?;
    let result = resolve_git_source_from_retained_cache_with_selection(
        primary_git,
        package_controlled_roots,
        request,
        git_lane.path(),
        git_lane.directory(),
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
    );
    let git_custody = git_lane.verify_path_identity();
    let member_custody = member_lane.verify_path_identity();
    git_custody?;
    member_custody?;
    result.map(|(source, evidence)| GitWorkspaceProjectionResult::new(source, evidence))
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
    validate_workspace_declaration_limits(limits, declaration_limits)?;
    let maximum_members = declaration_limits.maximum_members().min(limits.max_entries);
    let maximum_declaration_bytes = declaration_limits
        .maximum_declaration_bytes()
        .min(limits.max_bytes);
    let maximum_total_declaration_bytes = declaration_limits
        .maximum_total_declaration_bytes()
        .min(limits.max_bytes);
    let inspection = inspect_git_tree_graph(executor, repository, tree)?;
    debug_assert_eq!(inspection.root_tree_oid(), tree);

    let root_path = b"build.omg".to_vec();
    let mut root_entries = inspection.open_regular_files(
        executor,
        repository,
        [root_path.clone()],
        LocalSourceLimits {
            max_entries: 1,
            max_bytes: maximum_declaration_bytes,
            max_depth: limits.max_depth,
        },
    )?;
    let root_entry = root_entries
        .pop()
        .expect("one exact root declaration path yields one authenticated entry");
    let root_declaration = GitWorkspaceDeclaration::root(
        "build.omg".to_owned(),
        root_entry.oid.clone(),
        git_file_bytes(&root_entry)?.to_vec(),
    );
    let member_paths = planner
        .discover_members(&root_declaration)
        .map_err(GitWorkspaceProjectionError::Planner)?;
    if member_paths.len() > maximum_members {
        return Err(SourceResolveError::TooManyFiles {
            limit: maximum_members,
        }
        .into());
    }

    let mut unique_members = BTreeSet::new();
    let mut member_declaration_paths = Vec::with_capacity(member_paths.len());
    for member_path in &member_paths {
        if !unique_members.insert(member_path.clone()) {
            return Err(SourceResolveError::GitTreeInvalid {
                path: member_path.as_str().as_bytes().to_vec(),
                message: "workspace planner returned one member path more than once".to_owned(),
            }
            .into());
        }
        member_declaration_paths.push(format!("{}/build.omg", member_path.as_str()).into_bytes());
    }

    let root_bytes = u64::try_from(root_declaration.bytes().len()).map_err(|_| {
        SourceResolveError::TooManyBytes {
            limit: maximum_total_declaration_bytes,
        }
    })?;
    let remaining_declaration_bytes = maximum_total_declaration_bytes
        .checked_sub(root_bytes)
        .ok_or(SourceResolveError::TooManyBytes {
            limit: maximum_total_declaration_bytes,
        })?;
    let member_entries = inspection.open_regular_files(
        executor,
        repository,
        member_declaration_paths.clone(),
        LocalSourceLimits {
            max_entries: maximum_members,
            max_bytes: remaining_declaration_bytes,
            max_depth: limits.max_depth,
        },
    )?;
    let mut member_declarations = Vec::with_capacity(member_entries.len());
    for ((member_path, repository_path), entry) in member_paths
        .iter()
        .cloned()
        .zip(member_declaration_paths.iter())
        .zip(member_entries)
    {
        if entry.size > maximum_declaration_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: maximum_declaration_bytes,
            }
            .into());
        }
        let repository_path = std::str::from_utf8(repository_path)
            .expect("validated workspace member paths are UTF-8")
            .to_owned();
        member_declarations.push(GitWorkspaceDeclaration::member(
            member_path,
            repository_path,
            entry.oid.clone(),
            git_file_bytes(&entry)?.to_vec(),
        ));
    }

    let selection = planner
        .select_member(&root_declaration, &member_declarations)
        .map_err(GitWorkspaceProjectionError::Planner)?;
    let (selected_member_path, evidence) = selection.into_parts();
    if !unique_members.contains(&selected_member_path) {
        return Err(SourceResolveError::GitTreeInvalid {
            path: selected_member_path.as_str().as_bytes().to_vec(),
            message: "workspace planner selected a member absent from its discovered set"
                .to_owned(),
        }
        .into());
    }

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
    let (snapshot_root, local) = publish_git_member_snapshot(
        executor,
        member_lane,
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

fn validate_workspace_declaration_limits(
    source_limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
) -> Result<(), SourceResolveError> {
    if source_limits.max_entries == 0
        || source_limits.max_bytes == 0
        || declaration_limits.maximum_members() == 0
        || declaration_limits.maximum_declaration_bytes() == 0
        || declaration_limits.maximum_total_declaration_bytes()
            < declaration_limits.maximum_declaration_bytes()
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "workspace declaration and source limits must be nonzero and coherent"
                .to_owned(),
        });
    }
    Ok(())
}

fn git_file_bytes(entry: &GitTreeEntry) -> Result<&[u8], SourceResolveError> {
    match &entry.kind {
        GitTreeEntryKind::File { bytes, .. } => Ok(bytes.as_slice()),
        _ => Err(SourceResolveError::GitTreeInvalid {
            path: entry.relative_bytes.clone(),
            message: "authenticated declaration is not a regular file".to_owned(),
        }),
    }
}
