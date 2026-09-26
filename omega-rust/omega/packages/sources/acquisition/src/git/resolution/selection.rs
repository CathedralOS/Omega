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
}
