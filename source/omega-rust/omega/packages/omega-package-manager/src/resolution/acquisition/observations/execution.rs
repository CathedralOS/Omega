use omega_resolver_execution::{
    ResolverExecutionCompletionObservation, ResolverExecutionEndpointObservation,
    ResolverExecutionPhase,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitExecutableIdentity {
    pub(in crate::resolution::acquisition) path: PathBuf,
    pub(in crate::resolution::acquisition) content_identity: String,
}

impl GitExecutableIdentity {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitTransportExecutableIdentity {
    pub(in crate::resolution::acquisition) invocation_path: PathBuf,
    pub(in crate::resolution::acquisition) path: PathBuf,
    pub(in crate::resolution::acquisition) content_identity: String,
}

impl GitTransportExecutableIdentity {
    /// Exact path through which Git selects this transport executable.
    ///
    /// HTTPS uses the install-owned `git-remote-https` entry while `path()`
    /// names its canonical executable target. SSH is invoked directly through
    /// the canonical path, so both paths are normally equal.
    pub fn invocation_path(&self) -> &Path {
        &self.invocation_path
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }
}

/// Bounded result provenance for one successfully completed native Git
/// command. This is locally constructed observation, not an admission receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandExecutionObservation {
    pub(in crate::resolution::acquisition) phase: ResolverExecutionPhase,
    pub(in crate::resolution::acquisition) policy_identity: String,
    pub(in crate::resolution::acquisition) command_identity: String,
    pub(in crate::resolution::acquisition) status_code: Option<i32>,
    pub(in crate::resolution::acquisition) termination_signal: Option<i32>,
    pub(in crate::resolution::acquisition) stdout_length: u64,
    pub(in crate::resolution::acquisition) stdout_identity: String,
    pub(in crate::resolution::acquisition) stderr_length: u64,
    pub(in crate::resolution::acquisition) stderr_identity: String,
    pub(in crate::resolution::acquisition) endpoint_observation:
        Option<ResolverExecutionEndpointObservation>,
    pub(in crate::resolution::acquisition) completion: ResolverExecutionCompletionObservation,
}

impl GitCommandExecutionObservation {
    pub const fn phase(&self) -> ResolverExecutionPhase {
        self.phase
    }

    pub fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    pub fn command_identity(&self) -> &str {
        &self.command_identity
    }

    pub const fn status_code(&self) -> Option<i32> {
        self.status_code
    }

    pub const fn termination_signal(&self) -> Option<i32> {
        self.termination_signal
    }

    pub const fn stdout_length(&self) -> u64 {
        self.stdout_length
    }

    pub fn stdout_identity(&self) -> &str {
        &self.stdout_identity
    }

    pub const fn stderr_length(&self) -> u64 {
        self.stderr_length
    }

    pub fn stderr_identity(&self) -> &str {
        &self.stderr_identity
    }

    pub const fn endpoint_observation(&self) -> Option<&ResolverExecutionEndpointObservation> {
        self.endpoint_observation.as_ref()
    }

    pub const fn completion(&self) -> &ResolverExecutionCompletionObservation {
        &self.completion
    }
}
