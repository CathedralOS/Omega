//! Selection intent is separate from authenticated source custody.

use crate::error::SourceResolveError;
use crate::git::objects::authentication::verify_exact_git_revision;
use crate::git::request::GitSourceRequest;
use crate::identity::{GitCommitId, GitObjectIdAlgorithm, GitTreeId};
use crate::observations::resolved::GitAcquisitionPin;

use super::exact_revision::GitExactRevisionAcquisition;

pub(super) struct RecordedGitRevision {
    pub(super) commit: String,
    pub(super) tree: String,
    pub(super) algorithm: GitObjectIdAlgorithm,
    pub(super) acquisition: GitExactRevisionAcquisition,
}

impl RecordedGitRevision {
    pub(super) fn new(
        request: &GitSourceRequest,
        commit: &GitCommitId,
        tree: &GitTreeId,
        acquisition: GitExactRevisionAcquisition,
    ) -> Result<Self, SourceResolveError> {
        let commit_hex = commit.to_hex();
        if commit.algorithm() != tree.algorithm() {
            return Err(SourceResolveError::GitObjectInvalid {
                oid: commit_hex,
                message: "recorded commit and root tree use different object formats".to_owned(),
            });
        }
        verify_exact_git_revision(request.requested_revision(), &commit_hex)?;
        Ok(Self {
            commit: commit_hex,
            tree: tree.to_hex(),
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
    Recorded(&'a RecordedGitRevision),
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
            Self::Recorded(recorded) => Some(&recorded.commit),
        }
    }

    pub(super) fn expected_tree(self) -> Option<&'a str> {
        match self {
            Self::Ordinary(pin) => pin.map(GitAcquisitionPin::tree),
            Self::Recorded(recorded) => Some(&recorded.tree),
        }
    }

    /// A missing retained entry may be acquired only when the selection
    /// authorizes transport: an unpinned ordinary request, or a recorded
    /// revision explicitly admitted for fetch.
    pub(super) fn permits_new_acquisition(self) -> bool {
        match self {
            Self::Ordinary(pin) => pin.is_none(),
            Self::Recorded(recorded) => {
                recorded.acquisition == GitExactRevisionAcquisition::AllowFetch
            }
        }
    }

    /// The object format a created cache entry must assume, when the selection
    /// already names one. An ordinary request lets the fetched commit select
    /// its own format.
    pub(super) fn known_object_algorithm(self) -> Option<GitObjectIdAlgorithm> {
        match self {
            Self::Ordinary(_) => None,
            Self::Recorded(recorded) => Some(recorded.algorithm),
        }
    }

    /// The recorded revision's own absence failure, when the selection refuses
    /// transport and the retained cache holds no such entry.
    pub(super) fn offline_unavailable(self) -> Option<SourceResolveError> {
        match self {
            Self::Recorded(recorded)
                if recorded.acquisition == GitExactRevisionAcquisition::Offline =>
            {
                Some(recorded.unavailable())
            }
            Self::Ordinary(_) | Self::Recorded(_) => None,
        }
    }

    /// The revision a fetch of this selection targets: the recorded commit, or
    /// the request's own revision for ordinary resolution.
    pub(super) fn fetch_revision(self, requested_rev: &'a str) -> &'a str {
        match self {
            Self::Ordinary(_) => requested_rev,
            Self::Recorded(recorded) => recorded.commit.as_str(),
        }
    }
}
