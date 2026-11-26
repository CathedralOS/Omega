//! Check the selected commit and its tree in the retained repository before deciding whether to fetch.

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::authentication::authenticate_git_commit;
use crate::git::objects::{ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object};
use crate::identity::GitObjectIdAlgorithm;
use crate::limits::{GIT_CONFIG_SHA1, GIT_CONFIG_SHA256, LocalSourceLimits};

use super::exact_revision::GitExactRevisionAcquisition;
use super::selection::ExactGitRevision;

pub(super) fn exact_revision_needs_fetch(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    revision: &ExactGitRevision,
    limits: LocalSourceLimits,
) -> Result<bool, SourceResolveError> {
    repository.verify_current(limits)?;
    let expected_config = match revision.algorithm {
        GitObjectIdAlgorithm::Sha1 => GIT_CONFIG_SHA1,
        GitObjectIdAlgorithm::Sha256 => GIT_CONFIG_SHA256,
    };
    if repository.read_canonical_config()? != expected_config {
        return Err(SourceResolveError::GitObjectInvalid {
            oid: revision.commit.clone(),
            message: "recorded object format differs from the retained repository".to_owned(),
        });
    }
    let commit_present = present(
        executor,
        repository,
        &revision.commit,
        ExactGitObjectKind::Commit,
    )?;
    let tree = if commit_present {
        Some(authenticate_git_commit(
            executor,
            repository,
            &revision.commit,
            revision.tree.as_deref(),
        )?)
    } else {
        revision.tree.clone()
    };
    let tree_present = match tree {
        Some(tree) => present(executor, repository, &tree, ExactGitObjectKind::Tree)?,
        None => false,
    };
    repository.verify_current(limits)?;
    if commit_present && tree_present {
        return Ok(false);
    }
    match revision.acquisition {
        GitExactRevisionAcquisition::Offline => Err(revision.unavailable()),
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
