//! Preserve accepted repository revisions while selected sources are updated.

use crate::declarations::PackageKey;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalSourceClosureSubject,
};
use crate::resolution::source::ResolvePackageSourceError;
use omega_package_source::git::resolution::GitExactRevisionAcquisition;
use omega_package_source::{GitSourceRequest, ImmutableSourceResolution};
use std::fmt;

/// A borrowed resolution policy, not an acceptance or compiler-review result.
///
/// Unchanged locator/revision requests retain their recorded commit and tree.
/// Selected packages refresh their repository as a unit: Git workspace members
/// and their relative Path edges cannot use different commits of that repository.
/// New or changed requests resolve normally and still undergo graph reconciliation.
#[derive(Debug, Clone, Copy)]
pub struct GitDependencyPins<'a> {
    accepted: &'a CanonicalSourceClosureSubject,
    updates: &'a [PackageKey],
    acquisition: GitExactRevisionAcquisition,
}

impl<'a> GitDependencyPins<'a> {
    /// An empty selection preserves every unchanged Git request (installation).
    /// To update everything, use ordinary unpinned resolution instead.
    /// The acquisition setting controls missing *preserved* pins only; new and
    /// explicitly refreshed requests use the normal network resolver.
    pub fn new(
        accepted: &'a CanonicalSourceClosureSubject,
        updates: &'a [PackageKey],
        acquisition: GitExactRevisionAcquisition,
    ) -> Result<Self, GitDependencyPinsError> {
        for (index, package) in updates.iter().enumerate() {
            if accepted
                .packages()
                .binary_search_by(|source| source.key().cmp(package))
                .is_err()
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

    pub(super) fn accepted(&self) -> &CanonicalSourceClosureSubject {
        self.accepted
    }

    pub(super) fn acquisition(&self) -> GitExactRevisionAcquisition {
        self.acquisition
    }

    pub(super) fn resolution(
        &self,
        request: &GitSourceRequest,
    ) -> Result<Option<&ImmutableSourceResolution>, ResolvePackageSourceError> {
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
        if let CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
            ..
        } = self.accepted.root().request()
            && matches(requested_locator, requested_revision)
        {
            retained = Some(self.accepted.root().selected().resolution());
        }
        for edge in self.accepted.dependency_requests() {
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
pub enum GitDependencyPinsError {
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
