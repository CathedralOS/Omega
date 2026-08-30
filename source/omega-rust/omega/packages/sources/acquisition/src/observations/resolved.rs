use crate::git::request::GitTransportProfile;
use crate::git::workspace::GitWorkspaceProjectionCustody;
use crate::tree::ResolvedLocalSource;
use omega_resolver_execution::ResolverExecutionPolicyObservation;
use std::path::{Path, PathBuf};

use super::accounting::GitCapturedOutputObservation;
use super::execution::{GitCommandExecutionObservation, GitExecutableIdentity};
use super::resolution::GitSourceReceipt;
use super::storage::GitRetainedStorageObservation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub(crate) requested_locator: String,
    pub(crate) locator_identity: String,
    pub(crate) transport_profile: GitTransportProfile,
    pub(crate) requested_rev: String,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) materialized_tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    pub(crate) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    /// Absolute parent Git executable identity observed before and after every launch.
    /// This is diagnostic custody, not certification of the executable.
    pub(crate) git_executable: GitExecutableIdentity,
    /// Locally reconstructed native policy observations for every command
    /// configured during this resolution. These rows report which optional
    /// platform hardening was active; unavailable rows do not invalidate a
    /// successful source resolution.
    pub(crate) execution_policy_observations: Vec<ResolverExecutionPolicyObservation>,
    pub(crate) command_execution_observations: Vec<GitCommandExecutionObservation>,
    pub(crate) captured_output_observation: GitCapturedOutputObservation,
    pub(crate) retained_storage_observation: GitRetainedStorageObservation,
    pub(crate) receipt: GitSourceReceipt,
}

/// Operation-local reuse pin for one already resolved Git acquisition.
///
/// This is not lock evidence. It only prevents a later member selection in the
/// same manager traversal from refetching or drifting away from the exact
/// commit and root tree already authenticated for that request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitAcquisitionPin {
    requested_locator: String,
    locator_identity: String,
    transport_profile: GitTransportProfile,
    requested_rev: String,
    commit: String,
    tree: String,
}

impl ResolvedGitSource {
    pub fn acquisition_pin(&self) -> GitAcquisitionPin {
        GitAcquisitionPin {
            requested_locator: self.requested_locator.clone(),
            locator_identity: self.locator_identity.clone(),
            transport_profile: self.transport_profile,
            requested_rev: self.requested_rev.clone(),
            commit: self.commit.clone(),
            tree: self.tree.clone(),
        }
    }
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

    pub fn materialized_tree(&self) -> &str {
        &self.materialized_tree
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub const fn local(&self) -> &ResolvedLocalSource {
        &self.local
    }

    pub const fn workspace_projection(&self) -> Option<&GitWorkspaceProjectionCustody> {
        self.workspace_projection.as_ref()
    }

    pub const fn git_executable(&self) -> &GitExecutableIdentity {
        &self.git_executable
    }

    pub fn execution_policy_observations(&self) -> &[ResolverExecutionPolicyObservation] {
        &self.execution_policy_observations
    }

    pub fn command_execution_observations(&self) -> &[GitCommandExecutionObservation] {
        &self.command_execution_observations
    }

    pub const fn captured_output_observation(&self) -> &GitCapturedOutputObservation {
        &self.captured_output_observation
    }

    /// Exact post-resolution cache state accepted by the final capability-rooted
    /// custody walk. This does not claim a during-write disk quota.
    pub const fn retained_storage_observation(&self) -> &GitRetainedStorageObservation {
        &self.retained_storage_observation
    }

    /// Canonical receipt of one successful resolution, issued only after
    /// source, cache, executable, policy, command, and accounting reconciliation
    /// all succeed. It records the platform hardening that was actually present
    /// without claiming that ambient host authority was excluded.
    pub fn receipt(&self) -> &GitSourceReceipt {
        &self.receipt
    }
}

impl GitAcquisitionPin {
    pub(crate) fn matches_request(
        &self,
        requested_locator: &str,
        locator_identity: &str,
        transport_profile: GitTransportProfile,
        requested_rev: &str,
    ) -> bool {
        self.requested_locator == requested_locator
            && self.locator_identity == locator_identity
            && self.transport_profile == transport_profile
            && self.requested_rev == requested_rev
    }

    pub(crate) fn commit(&self) -> &str {
        &self.commit
    }

    pub(crate) fn tree(&self) -> &str {
        &self.tree
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
    pub(crate) materialized_tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    pub(crate) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    pub(crate) git_executable: GitExecutableIdentity,
    pub(crate) execution_policy_observations: Vec<ResolverExecutionPolicyObservation>,
    pub(crate) command_execution_observations: Vec<GitCommandExecutionObservation>,
    pub(crate) captured_output_observation: GitCapturedOutputObservation,
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
            materialized_tree: resolved.materialized_tree.clone(),
            snapshot_root: resolved.snapshot_root.clone(),
            local: resolved.local.clone(),
            workspace_projection: resolved.workspace_projection.clone(),
            git_executable: resolved.git_executable.clone(),
            execution_policy_observations: resolved.execution_policy_observations.clone(),
            command_execution_observations: resolved.command_execution_observations.clone(),
            captured_output_observation: resolved.captured_output_observation.clone(),
        }
    }
}
