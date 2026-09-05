use super::super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::super::errors::ResolveDependencySourceError;
use super::context::{
    GitRepositoryContext, GitRepositoryWorkspaceEvidence, WorkspaceContext, WorkspaceContextKind,
};
use crate::declarations::dependencies::read::PackageSelection;
use crate::resolution::source::{
    GitPackageSourceRequest, GitWorkspaceSelectionEvidence, PackageSourceCustody,
    PackageSourceSelectionEvidence, ResolvePackageSourceError,
};
use build_declarations::WorkspaceMemberPath;
use package_source::{
    GitSourceRequest, ImmutableSourceResolution, LocalSourceLimits, SourceLineage,
    WorkspaceLineageIdentity,
};
use std::collections::{BTreeMap, BTreeSet};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_git_dependency(
    repository: &str,
    revision: &str,
    selection: &PackageSelection,
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    git_acquisitions: &mut GitAcquisitionCache,
    git_cache: SourceCacheLane<'_>,
    workspace_cache: SourceCacheLane<'_>,
    source_limits: LocalSourceLimits,
) -> Result<PackageSourceCustody, ResolveDependencySourceError> {
    let request = GitPackageSourceRequest::new(
        GitSourceRequest::new(repository.to_owned(), Some(revision.to_owned()))?,
        selection.clone(),
    );
    let resolved =
        git_acquisitions.resolve_selected(&request, git_cache, workspace_cache, source_limits)?;
    register_git_repository(
        workspaces,
        request.acquisition(),
        resolved.key().source_lineage(),
        resolved.resolution(),
        resolved.selection_evidence(),
        resolved.source_limits(),
    )?;
    Ok(resolved.into_custody())
}

pub(in super::super) fn register_git_repository(
    workspaces: &mut BTreeMap<WorkspaceLineageIdentity, WorkspaceContext>,
    request: &GitSourceRequest,
    root_source: &SourceLineage,
    resolution: &ImmutableSourceResolution,
    selection_evidence: &PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
) -> Result<WorkspaceLineageIdentity, ResolveDependencySourceError> {
    let (declared_members, workspace_evidence) = match selection_evidence {
        PackageSourceSelectionEvidence::Root => (BTreeSet::new(), None),
        PackageSourceSelectionEvidence::GitWorkspace(evidence) => (
            evidence
                .plan()
                .members()
                .iter()
                .map(|member| member.member_path().clone())
                .collect(),
            Some(GitRepositoryWorkspaceEvidence {
                workspace: evidence.plan().workspace_evidence().clone(),
                declarations: evidence.declarations().clone(),
            }),
        ),
    };
    let identity = WorkspaceLineageIdentity::from_root_source(root_source)
        .map_err(ResolvePackageSourceError::from)?;
    let context = WorkspaceContext {
        root_source: root_source.clone(),
        kind: WorkspaceContextKind::Git(GitRepositoryContext {
            request: request.clone(),
            resolution: resolution.clone(),
            declared_members,
            workspace_evidence,
            source_limits: source_limits.compiler_bounded(),
        }),
    };
    if let Some(existing) = workspaces.get(&identity) {
        // Authored selectors may differ while selecting one immutable tree.
        // Keep the first request: Path members reuse its acquisition pin.
        let same_repository = matches!(
            (&existing.kind, &context.kind),
            (WorkspaceContextKind::Git(existing), WorkspaceContextKind::Git(candidate))
                if existing.resolution == candidate.resolution
                    && existing.declared_members == candidate.declared_members
                    && existing.workspace_evidence == candidate.workspace_evidence
                    && existing.source_limits == candidate.source_limits
        );
        if existing.root_source != context.root_source || !same_repository {
            return Err(ResolveDependencySourceError::ConflictingWorkspaceRoot { identity });
        }
    } else {
        workspaces.insert(identity.clone(), context);
    }
    Ok(identity)
}

pub(super) fn workspace_member_selection(
    repository: &GitRepositoryContext,
    member_path: &WorkspaceMemberPath,
) -> GitWorkspaceSelectionEvidence {
    repository
        .workspace_evidence
        .as_ref()
        .and_then(|evidence| {
            evidence
                .workspace
                .select_declared_member(member_path)
                .map(|plan| GitWorkspaceSelectionEvidence::new(plan, evidence.declarations.clone()))
        })
        .expect("declared Git member set and retained workspace selection are one custody value")
}
