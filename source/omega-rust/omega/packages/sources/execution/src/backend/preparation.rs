use super::observation::ResolverExecutionPolicyInputs;
use super::request::validate_launch_request;
use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::ResolverPreparedExecution;
#[cfg(target_os = "macos")]
use crate::confinement;
use crate::model::ResolverExecutionPhase;
#[cfg(test)]
use crate::model::ResolverExecutionPolicyObservation;
use crate::process::limits;
use std::io;
use std::path::Path;
#[cfg(any(test, not(target_os = "macos")))]
use std::process::Command;

impl ResolverExecutionBackend {
    /// Construct a command under the host's selected native enforcement.
    ///
    /// The phase is closed. Initialization and inspection retain local-only
    /// execution and write policy. Discovery and fetch deliberately allow the
    /// selected Git process to use its ordinary host-selected descendants,
    /// transport, authentication, and writable state.
    #[cfg(test)]
    pub(crate) fn command(
        &self,
        executable: &Path,
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<Command> {
        self.command_with_observation(executable, phase, mutable_root)
            .map(|(command, _observation)| command)
    }

    #[cfg(all(test, target_os = "macos"))]
    pub(crate) fn command_with_inspection_read_root(
        &self,
        executable: &Path,
        inspection_read_root: &Path,
    ) -> io::Result<Command> {
        let (mut command, _observation) =
            self.command_with_inspection_read_root_observation(executable, inspection_read_root)?;
        command.current_dir(inspection_read_root);
        Ok(command)
    }

    /// Prepare one phase command with its policy retained inside the opaque
    /// execution value. Spawning consumes that value.
    pub fn prepare(
        &self,
        executable: &Path,
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            phase,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: None,
                mutable_root,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_observation(
        &self,
        executable: &Path,
        phase: ResolverExecutionPhase,
        mutable_root: Option<&Path>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare(executable, phase, mutable_root)
            .map(ResolverPreparedExecution::into_parts)
    }

    /// Construct one repository-inspection command bound to the exact retained
    /// repository it inspects. Ambient host reads remain available to Git.
    pub fn prepare_inspection(
        &self,
        executable: &Path,
        inspection_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            ResolverExecutionPhase::RepositoryInspection,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: None,
                inspection_read_root: Some(inspection_read_root),
                mutable_root: None,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_inspection_read_root_observation(
        &self,
        executable: &Path,
        inspection_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_inspection(executable, inspection_read_root)
            .map(ResolverPreparedExecution::into_parts)
    }

    /// Construct one host-routed transport-discovery command bound to its
    /// exact working root.
    pub fn prepare_discovery(
        &self,
        executable: &Path,
        discovery_read_root: &Path,
    ) -> io::Result<ResolverPreparedExecution> {
        self.prepare_with_authority_roots(
            executable,
            ResolverExecutionPhase::TransportDiscovery,
            ResolverExecutionAuthorityRoots {
                discovery_read_root: Some(discovery_read_root),
                inspection_read_root: None,
                mutable_root: None,
            },
        )
    }

    #[cfg(test)]
    pub fn command_with_discovery_observation(
        &self,
        executable: &Path,
        discovery_read_root: &Path,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_discovery(executable, discovery_read_root)
            .map(ResolverPreparedExecution::into_parts)
    }

    pub(super) fn prepare_with_authority_roots(
        &self,
        executable: &Path,
        phase: ResolverExecutionPhase,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<ResolverPreparedExecution> {
        self.verify()?;
        validate_launch_request(executable, phase, roots)?;

        #[cfg(target_os = "macos")]
        let (mut command, generated_policy_sha256) =
            confinement::macos::command(self, executable, phase, roots)?;
        #[cfg(not(target_os = "macos"))]
        let mut command = Command::new(executable);
        #[cfg(not(target_os = "macos"))]
        let generated_policy_sha256 = None;

        limits::configure_child_resource_limits(&mut command)?;
        let observation = self.policy_observation(ResolverExecutionPolicyInputs {
            phase,
            generated_policy_sha256,
            executable,
            discovery_read_root: roots.discovery_read_root,
            inspection_read_root: roots.inspection_read_root,
            mutable_root: roots.mutable_root,
        })?;
        Ok(ResolverPreparedExecution::new(command, observation))
    }

    #[cfg(test)]
    pub(super) fn command_with_authority_roots_observation(
        &self,
        executable: &Path,
        phase: ResolverExecutionPhase,
        roots: ResolverExecutionAuthorityRoots<'_>,
    ) -> io::Result<(Command, ResolverExecutionPolicyObservation)> {
        self.prepare_with_authority_roots(executable, phase, roots)
            .map(ResolverPreparedExecution::into_parts)
    }
}
