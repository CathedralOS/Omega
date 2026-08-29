//! Transport-erased custody for one declared package snapshot.

use crate::manifest::dependency_projection::DependencySourceRequest;
use crate::source::LocalSourceLimits;
use crate::source::identity::{ImmutableSourceResolution, PackageKey};
use std::path::{Path, PathBuf};

/// Immutable package source after acquisition, declaration extraction, and
/// dependency projection have all succeeded.
#[derive(Debug, Clone)]
pub struct PackageSourceCustody {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    pub(crate) snapshot_root: PathBuf,
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
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
}
