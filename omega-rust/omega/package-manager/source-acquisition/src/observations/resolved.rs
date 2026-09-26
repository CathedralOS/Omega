use crate::git::request::GitTransportProfile;
use crate::git::workspace::GitWorkspaceProjectionCustody;
use crate::identity::{GitObjectIdAlgorithm, SourceLineage, SourceRelativePath};
use crate::limits::LocalSourceLimits;
use crate::tree::ResolvedLocalSource;
use std::path::{Path, PathBuf};

use super::storage::GitRetainedStorageCustody;

/// Direct custody for one resolved Git source.
///
/// Every field describes the authored request, accepted objects, materialized
/// source, immutable snapshot, or a concrete enforced bound. Fetch-process
/// telemetry is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub(crate) requested_locator: String,
    pub(crate) lineage: SourceLineage,
    pub(crate) locator_identity: String,
    pub(crate) transport_profile: GitTransportProfile,
    pub(crate) requested_rev: String,
    pub(crate) object_format: GitObjectIdAlgorithm,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) materialized_tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    pub(crate) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    pub(crate) source_limits: LocalSourceLimits,
    pub(crate) retained_storage: GitRetainedStorageCustody,
}

/// Operation-local reuse pin for one already resolved Git acquisition.
///
/// This is not lock evidence. It only prevents a later member selection in the
/// same manager traversal from refetching or drifting away from the exact
/// commit and root tree already authenticated for that request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitAcquisitionPin {
    requested_locator: String,
    lineage: SourceLineage,
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
            lineage: self.lineage.clone(),
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

    pub const fn lineage(&self) -> &SourceLineage {
        &self.lineage
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

    pub const fn object_format(&self) -> GitObjectIdAlgorithm {
        self.object_format
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

    pub fn selected_member(&self) -> Option<&SourceRelativePath> {
        self.workspace_projection
            .as_ref()
            .map(GitWorkspaceProjectionCustody::selected_member_path)
    }

    pub fn content_identity(&self) -> &str {
        &self.local.content_identity
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

    pub const fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub const fn retained_storage(&self) -> &GitRetainedStorageCustody {
        &self.retained_storage
    }
}

impl GitAcquisitionPin {
    pub(crate) fn matches_request(
        &self,
        requested_locator: &str,
        lineage: &SourceLineage,
        locator_identity: &str,
        transport_profile: GitTransportProfile,
        requested_rev: &str,
    ) -> bool {
        self.requested_locator == requested_locator
            && &self.lineage == lineage
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
    pub(crate) lineage: SourceLineage,
    pub(crate) locator_identity: String,
    pub(crate) transport_profile: GitTransportProfile,
    pub(crate) requested_rev: String,
    pub(crate) object_format: GitObjectIdAlgorithm,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) materialized_tree: String,
    pub(crate) snapshot_root: PathBuf,
    pub(crate) local: ResolvedLocalSource,
    pub(crate) workspace_projection: Option<GitWorkspaceProjectionCustody>,
    pub(crate) source_limits: LocalSourceLimits,
}

#[cfg(test)]
impl PendingResolvedGitSource {
    pub(crate) fn from_issued(resolved: &ResolvedGitSource) -> Self {
        Self {
            requested_locator: resolved.requested_locator.clone(),
            lineage: resolved.lineage.clone(),
            locator_identity: resolved.locator_identity.clone(),
            transport_profile: resolved.transport_profile,
            requested_rev: resolved.requested_rev.clone(),
            object_format: resolved.object_format,
            commit: resolved.commit.clone(),
            tree: resolved.tree.clone(),
            materialized_tree: resolved.materialized_tree.clone(),
            snapshot_root: resolved.snapshot_root.clone(),
            local: resolved.local.clone(),
            workspace_projection: resolved.workspace_projection.clone(),
            source_limits: resolved.source_limits,
        }
    }
}
