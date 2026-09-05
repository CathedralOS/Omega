//! Frozen Git path and concrete command, time, output, and process budgets.

use super::budget::GitCapturedOutputBudget;
use super::selection::PrimaryGitSelection;
use crate::SourceResolveError;
use crate::git::commands::capture::duration_millis;
use crate::git::request::GitExecutionTransport;
use crate::limits::{
    GIT_COMMAND_TIMEOUT, GIT_FIXED_COMMAND_ALLOWANCE, GIT_RESOLUTION_TIMEOUT, LocalSourceLimits,
};
use resolver_execution::ResolverExecutionBackend;
use std::cell::Cell;
#[cfg(all(test, unix))]
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Retain which ceiling supplied the child deadline, including its cleanup
/// reserve. Cleanup can complete before the resolution's wall-clock deadline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GitCommandDeadline {
    timeout: Duration,
    resolution_timeout: Option<Duration>,
}

impl GitCommandDeadline {
    pub(crate) fn new(remaining: Duration, resolution_timeout: Duration) -> Self {
        Self {
            timeout: GIT_COMMAND_TIMEOUT.min(remaining),
            resolution_timeout: (remaining <= GIT_COMMAND_TIMEOUT).then_some(resolution_timeout),
        }
    }

    pub(crate) fn duration(self) -> Duration {
        self.timeout
    }

    pub(crate) fn project_error(self, error: SourceResolveError) -> SourceResolveError {
        match (error, self.resolution_timeout) {
            (SourceResolveError::GitTimedOut { .. }, Some(timeout)) => {
                SourceResolveError::GitResolutionTimedOut {
                    timeout_millis: duration_millis(timeout),
                }
            }
            (error, _) => error,
        }
    }
}

#[derive(Debug)]
pub(crate) struct GitExecutor {
    pub(crate) execution_transport: GitExecutionTransport,
    pub(crate) started: Instant,
    pub(crate) timeout: Duration,
    pub(crate) launches: Cell<usize>,
    pub(crate) captured_output_budget: GitCapturedOutputBudget,
    pub(crate) maximum_launches: usize,
    pub(crate) execution_backend: ResolverExecutionBackend,
}

impl GitExecutor {
    pub(crate) fn selected(
        primary_git: &PrimaryGitSelection,
        execution_transport: GitExecutionTransport,
        limits: LocalSourceLimits,
        package_controlled_roots: &[PathBuf],
    ) -> Result<Self, SourceResolveError> {
        Self::from_primary_git(
            primary_git,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            git_resolution_captured_output_ceiling(limits),
            execution_transport,
            package_controlled_roots,
        )
    }

    #[cfg(all(test, unix))]
    pub(crate) fn open(path: &Path) -> Result<Self, SourceResolveError> {
        Self::open_with_budget_for_transport(
            path,
            GIT_FIXED_COMMAND_ALLOWANCE,
            GIT_RESOLUTION_TIMEOUT,
            git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
            GitExecutionTransport::File,
        )
    }

    #[cfg(all(test, unix))]
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

    #[cfg(all(test, unix))]
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

    #[cfg(all(test, unix))]
    pub(crate) fn open_with_budget_for_transport(
        path: &Path,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
        execution_transport: GitExecutionTransport,
    ) -> Result<Self, SourceResolveError> {
        let primary_git = PrimaryGitSelection::capture(Some(path), &[])?;
        Self::from_primary_git(
            &primary_git,
            maximum_launches,
            timeout,
            captured_output_ceiling,
            execution_transport,
            &[],
        )
    }

    fn from_primary_git(
        primary_git: &PrimaryGitSelection,
        maximum_launches: usize,
        timeout: Duration,
        captured_output_ceiling: u64,
        execution_transport: GitExecutionTransport,
        package_controlled_roots: &[PathBuf],
    ) -> Result<Self, SourceResolveError> {
        primary_git.verify_outside(package_controlled_roots)?;
        let execution_backend =
            ResolverExecutionBackend::open(primary_git.path(), package_controlled_roots).map_err(
                |error| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: error.to_string(),
                },
            )?;
        Ok(Self {
            execution_transport,
            started: Instant::now(),
            timeout,
            launches: Cell::new(0),
            captured_output_budget: GitCapturedOutputBudget::new(captured_output_ceiling),
            maximum_launches,
            execution_backend,
        })
    }

    pub(crate) fn begin_launch(&self) -> Result<GitCommandDeadline, SourceResolveError> {
        self.verify_budget()?;
        let launches = self.launches.get();
        if launches >= self.maximum_launches {
            return Err(SourceResolveError::GitResolutionCommandLimit {
                limit: self.maximum_launches,
            });
        }
        self.launches.set(launches + 1);
        Ok(GitCommandDeadline::new(
            self.remaining_time()?,
            self.timeout,
        ))
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

pub(crate) fn git_resolution_captured_output_ceiling(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_add(crate::limits::GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE)
        .min(crate::limits::GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT)
}

#[cfg(test)]
pub(crate) fn test_system_git_executor(
    transport: GitExecutionTransport,
) -> Result<GitExecutor, SourceResolveError> {
    let primary_git = PrimaryGitSelection::capture(None, &[])?;
    GitExecutor::selected(&primary_git, transport, LocalSourceLimits::default(), &[])
}
