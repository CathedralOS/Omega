use crate::git::request::GitTransportProfile;
use crate::local::model::ResolvedLocalSource;
use omega_resolver_execution::ResolverExecutionPolicyObservation;
use std::path::{Path, PathBuf};

use super::accounting::{GitCapturedOutputObservation, GitNetworkTransferObservation};
use super::execution::{
    GitCommandExecutionObservation, GitExecutableIdentity, GitTransportExecutableIdentity,
};
use super::resolution::GitSourceResolutionObservation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub(crate) requested_locator: String,
    pub(crate) locator_identity: String,
    pub(crate) transport_profile: GitTransportProfile,
    pub(crate) requested_rev: String,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    /// Absolute parent Git executable identity observed before and after every launch.
    /// This is diagnostic custody, not certification of the executable.
    pub(crate) git_executable: GitExecutableIdentity,
    /// Exact transport executable observed for HTTPS or SSH resolution.
    /// The test-only file adapter retains no transport executable here.
    pub(crate) transport_executable: Option<GitTransportExecutableIdentity>,
    /// Fixed platform executables admitted in addition to Git and the selected
    /// transport helper. Each identity binds its invocation path, canonical
    /// target, and exact content digest.
    pub(crate) execution_helper_executables: Vec<GitTransportExecutableIdentity>,
    /// Locally reconstructed native policy observations for every command
    /// configured during this resolution. These rows are provenance, not accepted source
    /// authority; strict admission must reject any unavailable required row.
    pub(crate) execution_policy_observations: Vec<ResolverExecutionPolicyObservation>,
    pub(crate) command_execution_observations: Vec<GitCommandExecutionObservation>,
    pub(crate) captured_output_observation: GitCapturedOutputObservation,
    pub(crate) network_transfer_observation: GitNetworkTransferObservation,
    pub(crate) resolution_observation: GitSourceResolutionObservation,
}

impl ResolvedGitSource {
    pub fn requested_locator(&self) -> &str {
        &self.requested_locator
    }

    pub fn locator_identity(&self) -> &str {
        &self.locator_identity
    }

    pub const fn transport_profile(&self) -> GitTransportProfile {
        self.transport_profile
    }

    pub fn requested_revision(&self) -> &str {
        &self.requested_rev
    }

    pub fn commit(&self) -> &str {
        &self.commit
    }

    pub fn tree(&self) -> &str {
        &self.tree
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub const fn local(&self) -> &ResolvedLocalSource {
        &self.local
    }

    pub const fn git_executable(&self) -> &GitExecutableIdentity {
        &self.git_executable
    }

    pub const fn transport_executable(&self) -> Option<&GitTransportExecutableIdentity> {
        self.transport_executable.as_ref()
    }

    pub fn execution_policy_observations(&self) -> &[ResolverExecutionPolicyObservation] {
        &self.execution_policy_observations
    }

    pub fn execution_helper_executables(&self) -> &[GitTransportExecutableIdentity] {
        &self.execution_helper_executables
    }

    pub fn command_execution_observations(&self) -> &[GitCommandExecutionObservation] {
        &self.command_execution_observations
    }

    pub const fn captured_output_observation(&self) -> &GitCapturedOutputObservation {
        &self.captured_output_observation
    }

    pub const fn network_transfer_observation(&self) -> &GitNetworkTransferObservation {
        &self.network_transfer_observation
    }

    /// Canonical final-result provenance issued only after source, cache,
    /// executable, policy, and command reconciliation all succeed.
    ///
    /// This is not a strict source receipt: it preserves unavailable native
    /// guarantees rather than converting them into accepted authority.
    pub fn resolution_observation(&self) -> &GitSourceResolutionObservation {
        &self.resolution_observation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingResolvedGitSource {
    pub(crate) requested_locator: String,
    pub(crate) locator_identity: String,
    pub(crate) transport_profile: GitTransportProfile,
    pub(crate) requested_rev: String,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    pub(crate) git_executable: GitExecutableIdentity,
    pub(crate) transport_executable: Option<GitTransportExecutableIdentity>,
    pub(crate) execution_helper_executables: Vec<GitTransportExecutableIdentity>,
    pub(crate) execution_policy_observations: Vec<ResolverExecutionPolicyObservation>,
    pub(crate) command_execution_observations: Vec<GitCommandExecutionObservation>,
    pub(crate) captured_output_observation: GitCapturedOutputObservation,
    pub(crate) network_transfer_observation: GitNetworkTransferObservation,
}

#[cfg(test)]
impl PendingResolvedGitSource {
    pub(crate) fn from_issued(resolved: &ResolvedGitSource) -> Self {
        Self {
            requested_locator: resolved.requested_locator.clone(),
            locator_identity: resolved.locator_identity.clone(),
            transport_profile: resolved.transport_profile,
            requested_rev: resolved.requested_rev.clone(),
            commit: resolved.commit.clone(),
            tree: resolved.tree.clone(),
            snapshot_root: resolved.snapshot_root.clone(),
            local: resolved.local.clone(),
            git_executable: resolved.git_executable.clone(),
            transport_executable: resolved.transport_executable.clone(),
            execution_helper_executables: resolved.execution_helper_executables.clone(),
            execution_policy_observations: resolved.execution_policy_observations.clone(),
            command_execution_observations: resolved.command_execution_observations.clone(),
            captured_output_observation: resolved.captured_output_observation.clone(),
            network_transfer_observation: resolved.network_transfer_observation.clone(),
        }
    }
}
