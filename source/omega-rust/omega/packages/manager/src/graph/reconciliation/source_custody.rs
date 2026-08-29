//! Exact root requests and immutable package-source custody.

use super::super::validation::ResolvedSourceIdentity;
use crate::manifest::dependency_projection::DependencySourceRequest;
use crate::source::identity::{
    ExternalSourceContext, ImmutableSourceResolution, PackageKey, SourceLineage,
    WorkspaceMemberPath,
};
use crate::source::{GitSourceRequest, LocalSourceLimits};
use std::path::{Path, PathBuf};

/// Transport-erased custody for one resolved immutable package source.
///
/// There is deliberately no public constructor. Source adapters derive this
/// value from `ResolvedPackageSource<S>` only after source custody, package
/// declaration extraction, and hermetic dependency projection have succeeded.
#[derive(Debug, Clone)]
pub struct PackageSourceCustody {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    pub(super) snapshot_root: PathBuf,
    /// Resolver work ceiling retained for later custody revalidation. This is
    /// operational policy, not package/source identity.
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
}

/// The exact request that selected the root of one resolved source closure.
///
/// Dependency requests are authored in a requester's `build.omg` and remain in
/// that requester's custody. The root has no requester, so its request must be
/// retained separately instead of being inferred from normalized lineage or
/// immutable resolution after traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRootSourceRequest {
    Git(GitSourceRequest),
    WorkspaceMember {
        workspace_root_source: SourceLineage,
        member_path: WorkspaceMemberPath,
        requested_workspace_root: PathBuf,
    },
    ExternalLocal {
        requested_root: PathBuf,
        source_context: ExternalSourceContext,
    },
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.resolution == other.resolution
            && self.snapshot_root == other.snapshot_root
            && self.dependency_requests == other.dependency_requests
    }
}

impl Eq for PackageSourceCustody {}

impl PackageSourceCustody {
    pub(crate) fn from_resolved_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
        snapshot_root: PathBuf,
        source_limits: LocalSourceLimits,
        dependency_requests: Vec<DependencySourceRequest>,
    ) -> Self {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self {
            key,
            resolution,
            snapshot_root,
            source_limits,
            dependency_requests,
        }
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub fn dependency_requests(&self) -> &[DependencySourceRequest] {
        &self.dependency_requests
    }

    pub(super) fn source_identity(&self) -> ResolvedSourceIdentity {
        ResolvedSourceIdentity::from_validated_parts(self.key.clone(), self.resolution.clone())
    }
}
