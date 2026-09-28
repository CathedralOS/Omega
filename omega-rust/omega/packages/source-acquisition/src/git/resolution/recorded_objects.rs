//! A healthy cache may lack a historical object without being corrupted.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::authenticate_git_commit;
use crate::git::objects::{ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object};
use crate::identity::GitObjectIdAlgorithm;
use crate::limits::{GIT_CONFIG_SHA1, GIT_CONFIG_SHA256, LocalSourceLimits};

use super::exact_revision::GitExactRevisionAcquisition;
use super::selection::RecordedGitRevision;

pub(super) fn recorded_revision_needs_fetch(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    recorded: &RecordedGitRevision,
    limits: LocalSourceLimits,
) -> Result<bool, SourceResolveError> {
    repository.verify_current(limits)?;
    let expected_config = match recorded.algorithm {
        GitObjectIdAlgorithm::Sha1 => GIT_CONFIG_SHA1,
        GitObjectIdAlgorithm::Sha256 => GIT_CONFIG_SHA256,
    };
    if repository.read_canonical_config()? != expected_config {
        return Err(SourceResolveError::GitObjectInvalid {
            oid: recorded.commit.clone(),
            message: "recorded object format differs from the retained repository".to_owned(),
        });
    }
    let commit_present = present(
        executor,
        repository,
        &recorded.commit,
        ExactGitObjectKind::Commit,
    )?;
    if commit_present {
        // A wrong expected tree is a mismatch, not permission to fetch.
        authenticate_git_commit(executor, repository, &recorded.commit, &recorded.tree)?;
    }
    let tree_present = present(
        executor,
        repository,
        &recorded.tree,
        ExactGitObjectKind::Tree,
    )?;
    repository.verify_current(limits)?;
    if commit_present && tree_present {
        return Ok(false);
    }
    match recorded.acquisition {
        GitExactRevisionAcquisition::Offline => Err(recorded.unavailable()),
        GitExactRevisionAcquisition::AllowFetch => Ok(true),
    }
}

fn present(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    identity: &str,
    expected: ExactGitObjectKind,
) -> Result<bool, SourceResolveError> {
    match probe_exact_git_object(executor, repository, identity)? {
        ExactGitObjectAvailability::Missing => Ok(false),
        ExactGitObjectAvailability::Present { kind, .. } if kind == expected => Ok(true),
        ExactGitObjectAvailability::Present { .. } => Err(SourceResolveError::GitObjectInvalid {
            oid: identity.to_owned(),
            message: "recorded object has the wrong exact Git object kind".to_owned(),
        }),
    }
}
