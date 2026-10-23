use super::git_pins::{GitResolutionOptions, pin_error};
use crate::resolution::source::{
    GitPackageSourceRequest, ResolvePackageSourceError, ResolvedPackageSource, SourceRootRole,
    resolve_external_local_source_in_lane, resolve_selected_git_source_at_revision_in_lanes,
    resolve_selected_git_source_from_pin_in_lanes, resolve_workspace_member_source_in_lane,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::storage::RetainedStorageLane;
use package_source::{ExternalSourceContext, SourceLineage, SourceRelativePath};
use package_source::{
    GitAcquisitionPin, GitSourceRequest, LocalSourceLimits, ResolvedGitSource,
    ResolvedLocalSnapshot, SourceResolver,
};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum SourceCacheLane<'a> {
    Retained(&'a RetainedStorageLane),
}

impl SourceCacheLane<'_> {
    /// Retained child lane persisting checked source indexes for custodies
    /// rooted under this lane.
    pub(super) fn checked_source_cache_dir(
        &self,
    ) -> Result<std::path::PathBuf, ResolvePackageSourceError> {
        let Self::Retained(lane) = self;
        Ok(lane
            .retain_child(crate::resolution::source::CHECKED_SOURCE_CACHE_LANE)?
            .path()
            .to_path_buf())
    }

    /// Retained child lane persisting compiled review records for custodies
    /// rooted under this lane.
    pub(super) fn compiled_review_cache_dir(
        &self,
    ) -> Result<std::path::PathBuf, ResolvePackageSourceError> {
        let Self::Retained(lane) = self;
        Ok(lane
            .retain_child(crate::resolution::source::COMPILED_REVIEW_CACHE_LANE)?
            .path()
            .to_path_buf())
    }
}

pub(super) struct GitAcquisitionCache<'a> {
    resolver: &'a SourceResolver,
    options: GitResolutionOptions<'a>,
    pins: Vec<(GitSourceRequest, GitAcquisitionPin)>,
    selected: Vec<(
        GitPackageSourceRequest,
        ResolvedPackageSource<ResolvedGitSource>,
    )>,
}

impl<'a> GitAcquisitionCache<'a> {
    pub(super) fn with_options(
        resolver: &'a SourceResolver,
        options: GitResolutionOptions<'a>,
    ) -> Self {
        Self {
            resolver,
            options,
            pins: Vec::new(),
            selected: Vec::new(),
        }
    }

    /// Resolve one dependency Git request through the shared acquisition cache.
    /// The cache serves dependency sources only, so a prior entry is reusable
    /// only when its declared role is a package.
    pub(super) fn resolve_selected(
        &mut self,
        request: &GitPackageSourceRequest,
        git_cache: SourceCacheLane<'_>,
        member_cache: SourceCacheLane<'_>,
        limits: LocalSourceLimits,
    ) -> Result<ResolvedPackageSource<ResolvedGitSource>, ResolvePackageSourceError> {
        let limits = limits.compiler_bounded();
        if let Some((_, resolved)) = self
            .selected
            .iter()
            .find(|(selected, _)| selected == request)
            && resolved.role() == crate::declarations::BuildDeclarationKind::Package
        {
            return Ok(resolved.clone());
        }
        let pin = self
            .pins
            .iter()
            .find(|(acquisition, _)| acquisition == request.acquisition())
            .map(|(_, pin)| pin);
        let SourceCacheLane::Retained(git_lane) = git_cache;
        let SourceCacheLane::Retained(member_lane) = member_cache;
        let recorded = self
            .options
            .pins
            .as_ref()
            .map(|policy| policy.resolution(request.acquisition()))
            .transpose()?
            .flatten();
        let resolved = if let Some(expected) = recorded {
            let package_source::ImmutableSourceResolution::Git { commit, tree, .. } = expected
            else {
                return Err(pin_error(
                    request.acquisition(),
                    "recorded resolution is not Git",
                ));
            };
            let acquisition = if self.options.offline {
                GitExactRevisionAcquisition::Offline
            } else {
                self.options
                    .pins
                    .as_ref()
                    .expect("recorded pin has a policy")
                    .acquisition()
            };
            let resolved = resolve_selected_git_source_at_revision_in_lanes(
                self.resolver,
                request,
                commit,
                tree,
                acquisition,
                git_lane,
                member_lane,
                limits,
                SourceRootRole::Package,
            )?;
            if resolved.resolution() != expected {
                return Err(pin_error(
                    request.acquisition(),
                    "fresh source content differs from the accepted resolution",
                ));
            }
            Ok(resolved)
        } else if self.options.offline {
            return Err(pin_error(
                request.acquisition(),
                "offline resolution forbids new or refreshed Git selection; an unchanged recorded pin is required",
            ));
        } else {
            resolve_selected_git_source_from_pin_in_lanes(
                self.resolver,
                request,
                pin,
                git_lane,
                member_lane,
                limits,
                SourceRootRole::Package,
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
}

pub(super) fn resolve_external_local_package_from_cache(
    source_root: impl AsRef<Path>,
    cache: SourceCacheLane<'_>,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    match cache {
        SourceCacheLane::Retained(lane) => resolve_external_local_source_in_lane(
            source_root,
            lane,
            limits,
            source_context,
            SourceRootRole::Package,
        ),
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
        SourceCacheLane::Retained(lane) => resolve_workspace_member_source_in_lane(
            workspace_root_source,
            member_path,
            live_workspace_root,
            lane,
            limits,
            SourceRootRole::Package,
        ),
    }
}
