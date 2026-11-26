//! A full commit fixes the object format and fetch target. A recorded tree, when supplied, must also match the commit.

use crate::error::SourceResolveError;
use crate::git::objects::authentication::verify_exact_git_revision;
use crate::git::request::GitSourceRequest;
use crate::identity::{GitCommitId, GitObjectIdAlgorithm, GitTreeId};
use crate::observations::resolved::GitAcquisitionPin;

use super::exact_revision::GitExactRevisionAcquisition;

pub(super) struct ExactGitRevision {
    pub(super) commit: String,
    pub(super) tree: Option<String>,
    pub(super) algorithm: GitObjectIdAlgorithm,
    pub(super) acquisition: GitExactRevisionAcquisition,
}

impl ExactGitRevision {
    pub(super) fn new(
        request: &GitSourceRequest,
        commit: &GitCommitId,
        tree: Option<&GitTreeId>,
        acquisition: GitExactRevisionAcquisition,
    ) -> Result<Self, SourceResolveError> {
        let commit_hex = commit.to_hex();
        if tree.is_some_and(|tree| commit.algorithm() != tree.algorithm()) {
            return Err(SourceResolveError::GitObjectInvalid {
                oid: commit_hex,
                message: "recorded commit and root tree use different object formats".to_owned(),
            });
        }
        verify_exact_git_revision(request.requested_revision(), &commit_hex)?;
        Ok(Self {
            commit: commit_hex,
            tree: tree.map(GitTreeId::to_hex),
            algorithm: commit.algorithm(),
            acquisition,
        })
    }

    pub(super) fn unavailable(&self) -> SourceResolveError {
        SourceResolveError::GitExactRevisionUnavailable {
            commit: self.commit.clone(),
            tree: self.tree.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum GitRevisionSelection<'a> {
    Ordinary(Option<&'a GitAcquisitionPin>),
    Exact(&'a ExactGitRevision),
}

impl<'a> GitRevisionSelection<'a> {
    pub(super) fn validate_request(
        self,
        request: &GitSourceRequest,
    ) -> Result<(), SourceResolveError> {
        if let Self::Ordinary(Some(pin)) = self
            && !pin.matches_request(
                request.requested_locator(),
                request.lineage(),
                request.locator_identity(),
                request.transport_profile(),
                request.requested_revision(),
            )
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git acquisition reuse pin does not match the exact source request"
                    .to_owned(),
            });
        }
        Ok(())
    }

    pub(super) fn expected_commit(self) -> Option<&'a str> {
        match self {
            Self::Ordinary(pin) => pin.map(GitAcquisitionPin::commit),
            Self::Exact(revision) => Some(&revision.commit),
        }
    }

    pub(super) fn expected_tree(self) -> Option<&'a str> {
        match self {
            Self::Ordinary(pin) => pin.map(GitAcquisitionPin::tree),
            Self::Exact(revision) => revision.tree.as_deref(),
        }
    }

    pub(super) fn permits_new_acquisition(self) -> bool {
        match self {
            Self::Ordinary(pin) => pin.is_none(),
            Self::Exact(revision) => {
                revision.acquisition == GitExactRevisionAcquisition::AllowFetch
            }
        }
    }

    pub(super) fn known_object_algorithm(self) -> Option<GitObjectIdAlgorithm> {
        match self {
            Self::Ordinary(_) => None,
            Self::Exact(revision) => Some(revision.algorithm),
        }
    }

    pub(super) fn offline_unavailable(self) -> Option<SourceResolveError> {
        match self {
            Self::Exact(revision)
                if revision.acquisition == GitExactRevisionAcquisition::Offline =>
            {
                Some(revision.unavailable())
            }
            Self::Ordinary(_) | Self::Exact(_) => None,
        }
    }

    pub(super) fn fetch_revision(self, requested_rev: &'a str) -> &'a str {
        match self {
            Self::Ordinary(_) => requested_rev,
            Self::Exact(revision) => revision.commit.as_str(),
        }
    }
}
