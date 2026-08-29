use crate::resolution::binding::{
    GitPackageSourceRequest, ResolvePackageSourceError, ResolvedPackageSource,
    bind_git_package_source, resolve_external_local_package_source_in_lane,
    resolve_external_local_project_source_in_lane, resolve_workspace_member_package_source_in_lane,
};
use omega_package_source::{ExternalSourceContext, SourceLineage, WorkspaceMemberPath};
use omega_package_source::{
    GitSourceRequest, LocalSourceLimits, ResolvedGitSource, ResolvedLocalSnapshot,
};
use omega_package_source::{RetainedStorageLane, resolve_git_source_in_lane};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum SourceCacheLane<'a> {
    Retained(&'a RetainedStorageLane),
}

#[derive(Default)]
pub(super) struct GitAcquisitionCache {
    resolved: Vec<(GitSourceRequest, ResolvedGitSource)>,
}

impl GitAcquisitionCache {
    pub(super) fn resolve_selected(
        &mut self,
        request: &GitPackageSourceRequest,
        cache: SourceCacheLane<'_>,
        limits: LocalSourceLimits,
    ) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
        let limits = limits.compiler_bounded();
        let source = if let Some((_, source)) = self
            .resolved
            .iter()
            .find(|(acquisition, _)| acquisition == request.acquisition())
        {
            source.clone()
        } else {
            let SourceCacheLane::Retained(lane) = cache;
            let source = resolve_git_source_in_lane(request.acquisition(), lane, limits)?;
            self.resolved
                .push((request.acquisition().clone(), source.clone()));
            source
        };
        bind_git_package_source(
            request.acquisition().lineage().clone(),
            source,
            limits,
            request.selection(),
        )
    }

    #[cfg(test)]
    pub(super) fn acquisition_count(&self) -> usize {
        self.resolved.len()
    }
}

pub(super) fn resolve_external_local_package_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_package_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

pub(super) fn resolve_external_local_project_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => {
            resolve_external_local_project_source_in_lane(source_root, lane, limits, source_context)
        }
    }
}

pub(super) fn resolve_workspace_member_from_cache(
    workspace_root_source: &SourceLineage,
    member_path: WorkspaceMemberPath,
    live_workspace_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => resolve_workspace_member_package_source_in_lane(
            workspace_root_source,
            member_path,
            live_workspace_root,
            lane,
            limits,
        ),
    }
}
