use crate::resolution::source::{
    GitPackageSourceRequest, ResolvePackageSourceError, ResolvedPackageSource,
    resolve_external_local_package_source_in_lane, resolve_external_local_project_source_in_lane,
    resolve_selected_git_package_source_from_pin_in_lanes,
    resolve_selected_git_project_source_from_pin_in_lanes,
    resolve_workspace_member_package_source_in_lane,
    resolve_workspace_member_project_source_in_lane,
};
use omega_package_source::storage::RetainedStorageLane;
use omega_package_source::{ExternalSourceContext, SourceLineage, SourceRelativePath};
use omega_package_source::{
    GitAcquisitionPin, GitSourceRequest, LocalSourceLimits, ResolvedGitSource,
    ResolvedLocalSnapshot,
};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum SourceCacheLane<'a> {
    Retained(&'a RetainedStorageLane),
}

#[derive(Default)]
pub(super) struct GitAcquisitionCache {
    pins: Vec<(GitSourceRequest, GitAcquisitionPin)>,
    selected: Vec<(
        GitPackageSourceRequest,
        ResolvedPackageSource<ResolvedGitSource>,
    )>,
}

impl GitAcquisitionCache {
    pub(super) fn resolve_selected(
        &mut self,
        request: &GitPackageSourceRequest,
        git_cache: SourceCacheLane<'_>,
        member_cache: SourceCacheLane<'_>,
        limits: LocalSourceLimits,
    ) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
        self.resolve_selected_with_role(request, git_cache, member_cache, limits, false)
    }

    pub(super) fn resolve_selected_project(
        &mut self,
        request: &GitPackageSourceRequest,
        git_cache: SourceCacheLane<'_>,
        member_cache: SourceCacheLane<'_>,
        limits: LocalSourceLimits,
    ) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
        self.resolve_selected_with_role(request, git_cache, member_cache, limits, true)
    }

    fn resolve_selected_with_role(
        &mut self,
        request: &GitPackageSourceRequest,
        git_cache: SourceCacheLane<'_>,
        member_cache: SourceCacheLane<'_>,
        limits: LocalSourceLimits,
        application_root_allowed: bool,
    ) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
        let limits = limits.compiler_bounded();
        if let Some((_, resolved)) = self
            .selected
            .iter()
            .find(|(selected, _)| selected == request)
        {
            if application_root_allowed
                || resolved.role() == crate::manifest::BuildDeclarationKind::Package
            {
                return Ok(resolved.clone());
            }
        }
        let pin = self
            .pins
            .iter()
            .find(|(acquisition, _)| acquisition == request.acquisition())
            .map(|(_, pin)| pin);
        let SourceCacheLane::Retained(git_lane) = git_cache;
        let SourceCacheLane::Retained(member_lane) = member_cache;
        let resolved = if application_root_allowed {
            resolve_selected_git_project_source_from_pin_in_lanes(
                request,
                pin,
                git_lane,
                member_lane,
                limits,
            )
        } else {
            resolve_selected_git_package_source_from_pin_in_lanes(
                request,
                pin,
                git_lane,
                member_lane,
                limits,
            )
        }?;
        if pin.is_none() {
            self.pins.push((
                request.acquisition().clone(),
                resolved.source().acquisition_pin(),
            ));
        }
        self.selected.push((request.clone(), resolved.clone()));
        Ok(resolved)
    }

    #[cfg(test)]
    pub(super) fn acquisition_count(&self) -> usize {
        self.pins.len()
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
    member_path: SourceRelativePath,
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

pub(super) fn resolve_workspace_member_project_from_cache(
    workspace_root_source: &SourceLineage,
    member_path: SourceRelativePath,
    live_workspace_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => resolve_workspace_member_project_source_in_lane(
            workspace_root_source,
            member_path,
            live_workspace_root,
            lane,
            limits,
        ),
    }
}
