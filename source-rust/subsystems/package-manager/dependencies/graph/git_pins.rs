// Git selection policy (`GitResolutionOptions` and `GitDependencyPins`) preserving accepted repository revisions for unselected packages while refreshing selected repositories as a unit.

use crate::declarations::PackageKey;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalRootSourceRequest, CanonicalSourceClosureSubject,
    ResolvedSourceIdentity,
};
use crate::resolution::source::ResolvePackageSourceError;
use source_acquisition::git::resolution::GitExactRevisionAcquisition;
use source_acquisition::{GitCommitId, GitSourceRequest, GitTreeId, ImmutableSourceResolution};
use std::fmt;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct GitResolutionOptions<'a> {
    pub pins: Option<GitDependencyPins<'a>>,
    pub offline: bool,
}

pub(crate) struct ExactGitSource {
    pub commit: GitCommitId,
    pub tree: Option<GitTreeId>,
    pub acquisition: GitExactRevisionAcquisition,
}

impl<'a> GitResolutionOptions<'a> {
    pub(crate) fn exact_revision(
        &self,
        request: &GitSourceRequest,
    ) -> Result<Option<ExactGitSource>, ResolvePackageSourceError> {
        if let Some(policy) = &self.pins
            && let Some(resolution) = policy.resolution(request)?
        {
            let ImmutableSourceResolution::Git { commit, tree, .. } = resolution else {
                return Err(pin_error(request, "recorded Git source has a non-Git resolution"));
            };
            return Ok(Some(ExactGitSource {
                commit: commit.clone(),
                tree: Some(tree.clone()),
                acquisition: if self.offline {
                    GitExactRevisionAcquisition::Offline
                } else {
                    policy.acquisition
                },
            }));
        }
        if self.pins.is_none()
            && let Ok(commit) = GitCommitId::parse_hex(request.requested_revision())
        {
            return Ok(Some(ExactGitSource {
                commit,
                tree: None,
                acquisition: if self.offline {
                    GitExactRevisionAcquisition::Offline
                } else {
                    GitExactRevisionAcquisition::AllowFetch
                },
            }));
        }
        if self.offline {
            let reason = if self.pins.is_none() {
                "offline source selection requires a full commit ID in the Build declaration"
            } else {
                "offline lock resolution requires an unchanged recorded Git pin"
            };
            return Err(pin_error(request, reason));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_without_a_lock_requires_a_full_authored_commit() {
        let options = GitResolutionOptions {
            pins: None,
            offline: true,
        };
        let hex = "72bc01f59de8a314c60155e8c3b7474b261490fe";
        let request = |rev: &str| {
            GitSourceRequest::new("https://example.org/board.git", Some(rev.to_owned())).unwrap()
        };
        let selected = options.exact_revision(&request(hex)).unwrap().unwrap();
        assert_eq!(selected.commit.to_hex(), hex);
        assert_eq!(selected.tree, None);
        assert_eq!(selected.acquisition, GitExactRevisionAcquisition::Offline);
        let online = GitResolutionOptions::default();
        let fetched = online.exact_revision(&request(hex)).unwrap().unwrap();
        assert_eq!(fetched.commit, selected.commit);
        assert_eq!(fetched.tree, None);
        assert_eq!(fetched.acquisition, GitExactRevisionAcquisition::AllowFetch);
        for floating in ["stable", "refs/heads/stable", &hex[..12]] {
            let Err(error) = options.exact_revision(&request(floating)) else {
                panic!("accepted floating revision {floating} offline");
            };
            assert!(error.to_string().contains("full commit ID"), "{error}");
            assert!(online.exact_revision(&request(floating)).unwrap().is_none());
        }
    }
}
