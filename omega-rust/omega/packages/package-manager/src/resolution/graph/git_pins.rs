// Git selection policy (`GitResolutionOptions` and `GitDependencyPins`) preserving accepted repository revisions for unselected packages while refreshing selected repositories as a unit.

use crate::declarations::PackageKey;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalSourceClosureSubject,
    ResolvedSourceIdentity,
};
use crate::resolution::source::ResolvePackageSourceError;
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{GitSourceRequest, ImmutableSourceResolution};
use std::fmt;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct GitResolutionOptions<'a> {
    pub pins: Option<GitDependencyPins<'a>>,
    pub offline: bool,
}

pub(crate) struct RecordedGitSource<'a> {
    pub resolution: &'a ImmutableSourceResolution,
    pub acquisition: GitExactRevisionAcquisition,
}

impl<'a> GitResolutionOptions<'a> {
    pub(crate) fn recorded(
        &self,
        request: &GitSourceRequest,
    ) -> Result<Option<RecordedGitSource<'a>>, ResolvePackageSourceError> {
        if let Some(policy) = &self.pins
            && let Some(resolution) = policy.resolution(request)?
        {
            return Ok(Some(RecordedGitSource {
                resolution,
                acquisition: if self.offline {
                    GitExactRevisionAcquisition::Offline
                } else {
                    policy.acquisition
                },
            }));
        }
        if self.offline {
            return Err(pin_error(
                request,
                "offline resolution forbids new or refreshed Git selection; an unchanged recorded pin is required",
            ));
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GitDependencyPins<'a> {
    accepted: &'a CanonicalSourceClosureSubject,
    updates: &'a [PackageKey],
    acquisition: GitExactRevisionAcquisition,
}

impl<'a> GitDependencyPins<'a> {
    pub fn new(
        accepted: &'a CanonicalSourceClosureSubject,
        updates: &'a [PackageKey],
        acquisition: GitExactRevisionAcquisition,
    ) -> Result<Self, GitDependencyPinsError> {
        for (index, package) in updates.iter().enumerate() {
            if !accepted
                .execution_source_graphs()
                .any(|graph| ResolvedSourceIdentity::index_in(graph.packages(), package).is_some())
            {
                return Err(GitDependencyPinsError::UnknownPackage(package.clone()));
            }
            if updates[..index].contains(package) {
                return Err(GitDependencyPinsError::DuplicatePackage(package.clone()));
            }
        }
        Ok(Self {
            accepted,
            updates,
            acquisition,
        })
    }

    fn resolution(
        &self,
        request: &GitSourceRequest,
    ) -> Result<Option<&'a ImmutableSourceResolution>, ResolvePackageSourceError> {
        if self
            .updates
            .iter()
            .any(|package| package.source_lineage() == request.lineage())
        {
            return Ok(None);
        }
        let matches = |locator: &str, revision: &str| {
            locator == request.requested_locator() && revision == request.requested_revision()
        };
        let mut retained = None;
        for graph in self.accepted.execution_source_graphs() {
            if let CanonicalRootSourceRequest::Git {
                requested_locator,
                requested_revision,
                ..
            } = graph.root().request()
                && matches(requested_locator, requested_revision)
            {
                let resolution = graph.root().selected().resolution();
                if retained.is_some_and(|previous| previous != resolution) {
                    return Err(pin_error(
                        request,
                        "accepted requests disagree on the repository revision",
                    ));
                }
                retained = Some(resolution);
            }
            for edge in graph.dependency_requests() {
                if let CanonicalDependencySourceRequest::Git {
                    repository,
                    revision,
                    ..
                } = edge.request()
                    && matches(repository, revision)
                {
                    let resolution = edge.selected().resolution();
                    if retained.is_some_and(|previous| previous != resolution) {
                        return Err(pin_error(
                            request,
                            "accepted requests disagree on the repository revision",
                        ));
                    }
                    retained = Some(resolution);
                }
            }
        }
        Ok(retained)
    }
}

pub(super) fn pin_error(request: &GitSourceRequest, message: &str) -> ResolvePackageSourceError {
    ResolvePackageSourceError::RecordedGitPin {
        locator: request.requested_locator().to_owned(),
        revision: request.requested_revision().to_owned(),
        message: message.to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GitDependencyPinsError {
    UnknownPackage(PackageKey),
    DuplicatePackage(PackageKey),
}

impl fmt::Display for GitDependencyPinsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPackage(package) => write!(
                formatter,
                "update package `{}` is absent from the accepted graph",
                package.name().as_str()
            ),
            Self::DuplicatePackage(package) => write!(
                formatter,
                "update package `{}` was selected more than once",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for GitDependencyPinsError {}
