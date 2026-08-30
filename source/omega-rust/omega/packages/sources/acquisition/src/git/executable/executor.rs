//! Git executor lifecycle and joined policy/result observations.

use super::budget::GitCapturedOutputBudget;
use super::custody::verify_git_executable_custody;
use super::identity::{
    hash_git_executable, observe_git_executable_metadata, GitExecutableMetadataIdentity,
};
#[cfg(test)]
use super::selection::PrimaryGitSelection;
use crate::git::commands::capture::{duration_millis, BoundedCommandOutput};
use crate::git::request::GitExecutionTransport;
use crate::identity::digest::format_sha256;
use crate::limits::{
    LocalSourceLimits, GIT_COMMAND_TIMEOUT, GIT_FIXED_COMMAND_ALLOWANCE, GIT_RESOLUTION_TIMEOUT,
};
use crate::observations::accounting::{
    git_captured_output_observation, git_resolution_captured_output_ceiling,
    GitCapturedOutputObservation,
};
use crate::observations::execution::{
    GitCommandExecutionObservation, GitCommandInputCommitment, GitExecutableIdentity,
};
use crate::SourceResolveError;
use omega_resolver_execution::{
    ResolverExecutionBackend, ResolverExecutionPhase, ResolverExecutionPolicyObservation,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct GitExecutor {
    pub(crate) identity: GitExecutableIdentity,
    pub(crate) metadata_identity: GitExecutableMetadataIdentity,
    pub(crate) execution_transport: GitExecutionTransport,
    pub(crate) started: Instant,
    pub(crate) timeout: Duration,
    pub(crate) launches: Cell<usize>,
    pub(crate) execution_policy_observations: RefCell<Vec<ResolverExecutionPolicyObservation>>,
    pub(crate) command_execution_observations: RefCell<Vec<GitCommandExecutionObservation>>,
    pub(crate) captured_output_budget: GitCapturedOutputBudget,
    pub(crate) maximum_launches: usize,
    pub(crate) execution_backend: ResolverExecutionBackend,
}

impl GitExecutor {
    pub(crate) fn selected(
        path: &Path,
        execution_transport: GitExecutionTransport,
        limits: LocalSourceLimits,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            git_resolution_captured_output_ceiling(limits),
            execution_transport,
        )
    }

    #[cfg(test)]
    pub(crate) fn open(path: &Path) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
        )
    }

    #[cfg(test)]
    pub(crate) fn open_with_budget(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            maximum_launches,
            timeout,
            git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
        )
    }

    #[cfg(test)]
    pub(crate) fn open_with_resource_budgets(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
    ) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            maximum_launches,
            timeout,
            captured_output_ceiling,
            GitExecutionTransport::File,
        )
    }

    pub(crate) fn open_with_budget_for_transport(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
        execution_transport: GitExecutionTransport,
    ) -> Result<Self, SourceResolveError> {
        let started = Instant::now();
        if !path.is_absolute() {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: "path is not absolute".to_owned(),
            });
        }
        let canonical =
            path.canonicalize()
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        verify_git_executable_custody(&canonical)?;
        let metadata_identity = observe_git_executable_metadata(&canonical)?;
        let content_identity = hash_git_executable(&canonical)?;
        if observe_git_executable_metadata(&canonical)? != metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged { path: canonical });
        }
        let execution_backend = ResolverExecutionBackend::open().map_err(|error| {
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            identity: GitExecutableIdentity {
                path: canonical,
                content_identity,
            },
            metadata_identity,
            execution_transport,
            started,
            timeout,
            launches: Cell::new(0),
            execution_policy_observations: RefCell::new(Vec::new()),
            command_execution_observations: RefCell::new(Vec::new()),
            captured_output_budget: GitCapturedOutputBudget::new(captured_output_ceiling),
            maximum_launches,
            execution_backend,
        })
    }

    pub(crate) fn verify(&self) -> Result<(), SourceResolveError> {
        if observe_git_executable_metadata(&self.identity.path)? != self.metadata_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        verify_git_executable_custody(&self.identity.path)?;
        self.execution_backend.verify().map_err(|error| {
            SourceResolveError::GitExecutionBoundaryInvalid {
                message: error.to_string(),
            }
        })?;
        Ok(())
    }

    pub(crate) fn verify_content(&self) -> Result<(), SourceResolveError> {
        self.verify()?;
        if hash_git_executable(&self.identity.path)? != self.identity.content_identity {
            return Err(SourceResolveError::GitExecutableChanged {
                path: self.identity.path.clone(),
            });
        }
        self.verify()?;
        self.verify_budget()
    }

    pub(crate) fn validate_execution_policy_observations(&self) -> Result<(), SourceResolveError> {
        let observations = self.execution_policy_observations.borrow();
        if observations.len() != self.launches.get() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native policy observation count does not match launched command count"
                    .to_owned(),
            });
        }
        let command_observations = self.command_execution_observations.borrow();
        if command_observations.len() != self.launches.get() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native command outcome count does not match launched command count"
                    .to_owned(),
            });
        }
        for observation in observations.iter() {
            if observation.executable() != self.identity.path {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native policy observation names a different Git executable"
                        .to_owned(),
                });
            }
        }
        for (policy, command) in observations.iter().zip(command_observations.iter()) {
            if command.phase != policy.phase()
                || command.completion.policy() != policy
                || command.policy_identity
                    != format_sha256(&Sha256::digest(policy.canonical_bytes()))
            {
                return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "native command outcome is not joined to its policy observation"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn captured_output_observation(
        &self,
    ) -> Result<GitCapturedOutputObservation, SourceResolveError> {
        let expected = git_captured_output_observation(
            &self.command_execution_observations.borrow(),
            self.captured_output_budget.ceiling,
        )?;
        if expected.observed != self.captured_output_budget.observed() {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git captured-output counter does not match retained command outcomes"
                    .to_owned(),
            });
        }
        Ok(expected)
    }

    pub(crate) fn record_command_execution(
        &self,
        phase: ResolverExecutionPhase,
        command_identity: String,
        input: GitCommandInputCommitment,
        output: &BoundedCommandOutput,
    ) -> Result<(), SourceResolveError> {
        let completion = &output.completion;
        let policy = completion.policy();
        if policy.phase() != phase {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "native command phase does not match its completed policy observation"
                    .to_owned(),
            });
        }
        #[cfg(unix)]
        let termination_signal = {
            use std::os::unix::process::ExitStatusExt;
            output.status.signal()
        };
        #[cfg(not(unix))]
        let termination_signal = None;
        if completion.status().success() != output.status.success()
            || completion.status().code() != output.status.code()
            || completion.status().unix_signal() != termination_signal
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "captured Git status does not match resolver completion".to_owned(),
            });
        }
        let policy_identity = format_sha256(&Sha256::digest(policy.canonical_bytes()));
        let observation = GitCommandExecutionObservation {
            phase,
            policy_identity,
            command_identity,
            input,
            status_code: output.status.code(),
            termination_signal,
            stdout_length: output.stdout.len() as u64,
            stdout_identity: format_sha256(&Sha256::digest(&output.stdout)),
            stderr_length: output.stderr.len() as u64,
            stderr_identity: format_sha256(&Sha256::digest(&output.stderr)),
            completion: completion.clone(),
        };
        self.execution_policy_observations
            .borrow_mut()
            .push(policy.clone());
        self.command_execution_observations
            .borrow_mut()
            .push(observation);
        Ok(())
    }

    pub(crate) fn begin_launch(&self) -> Result<Duration, SourceResolveError> {
        self.verify_budget()?;
        let launches = self.launches.get();
        if launches >= self.maximum_launches {
            return Err(SourceResolveError::GitResolutionCommandLimit {
                limit: self.maximum_launches,
            });
        }
        self.launches.set(launches + 1);
        Ok(GIT_COMMAND_TIMEOUT.min(self.remaining_time()?))
    }

    pub(crate) fn verify_budget(&self) -> Result<(), SourceResolveError> {
        self.remaining_time().map(|_| ())
    }

    pub(crate) fn remaining_time(&self) -> Result<Duration, SourceResolveError> {
        let elapsed = self.started.elapsed();
        if elapsed >= self.timeout {
            Err(SourceResolveError::GitResolutionTimedOut {
                timeout_millis: duration_millis(self.timeout),
            })
        } else {
            Ok(self.timeout - elapsed)
        }
    }
}

impl crate::custody::lock::CacheLockBudget for GitExecutor {
    fn verify_cache_lock_budget(&self) -> Result<(), SourceResolveError> {
        self.verify_budget()
    }

    fn remaining_cache_lock_time(&self) -> Result<Duration, SourceResolveError> {
        self.remaining_time()
    }
}

#[cfg(test)]
pub(crate) fn test_system_git_executor(
    transport: GitExecutionTransport,
) -> Result<GitExecutor, SourceResolveError> {
    let selection = test_primary_git_selection()?;
    GitExecutor::selected(selection.path(), transport, LocalSourceLimits::default())
}

#[cfg(all(test, target_os = "macos"))]
fn test_primary_git_selection() -> Result<PrimaryGitSelection, SourceResolveError> {
    let output = std::process::Command::new("/usr/bin/xcrun")
        .args(["--find", "git"])
        .output()
        .map_err(|_| SourceResolveError::GitExecutableUnavailable)?;
    if !output.status.success() {
        return Err(SourceResolveError::GitExecutableUnavailable);
    }
    let path = std::str::from_utf8(&output.stdout)
        .map_err(|_| SourceResolveError::GitExecutableUnavailable)?
        .trim();
    PrimaryGitSelection::from_operator_or_environment(Some(Path::new(path)), &[])?
        .ok_or(SourceResolveError::GitExecutableUnavailable)
}

#[cfg(all(test, not(target_os = "macos")))]
fn test_primary_git_selection() -> Result<PrimaryGitSelection, SourceResolveError> {
    PrimaryGitSelection::from_operator_or_environment(None, &[])?
        .ok_or(SourceResolveError::GitExecutableUnavailable)
}
