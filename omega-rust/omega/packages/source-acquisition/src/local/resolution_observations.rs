// Value types for captured local sources, published local snapshots, and
// resolution observations.
//
// `CapturedLocalSource` holds an invocation-owned in-memory source tree; its
// `requested_root` is preserved for provenance and routing only, never as
// permission to reopen the live checkout. `LocalSourceResolutionObservation` is
// issued exclusively by the resolver to bind the caller's request spelling,
// canonical live root, immutable publication, limits, custody identity, and
// final exact-tree rehash. `ResolvedLocalSnapshot` pairs that observation with
// the published snapshot root and normalized tree identity.

use crate::tree::ResolvedSourceTree;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CapturedLocalSource {
    pub(crate) requested_root: PathBuf,
    pub(crate) normalized: ResolvedSourceTree,
    pub(crate) tree: Arc<source::CapturedSourceTree>,
    pub(crate) selection_identity: [u8; 32], // Caller inventory commitment, independent of physical root lineage.
}

impl CapturedLocalSource {
    pub const fn selection_identity(&self) -> &[u8; 32] {
        &self.selection_identity
    }

    pub fn requested_root(&self) -> &Path {
        &self.requested_root
    }
    pub fn canonical_live_root(&self) -> &Path {
        &self.normalized.root
    }
    pub fn normalized(&self) -> &ResolvedSourceTree {
        &self.normalized
    }
    pub fn tree(&self) -> &Arc<source::CapturedSourceTree> {
        &self.tree
    }
}

pub use crate::tree::{
    ResolvedLocalSource, VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSourceResolutionObservation {
    pub(super) schema_version: u32,
    pub(super) identity: String,
    pub(super) custody_identity: String,
}

impl LocalSourceResolutionObservation {
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[cfg(test)]
    pub(crate) fn custody_identity(&self) -> &str {
        &self.custody_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSnapshot {
    requested_root: PathBuf,
    canonical_live_root: PathBuf,
    snapshot_root: PathBuf,
    normalized: ResolvedSourceTree,
    resolution_observation: LocalSourceResolutionObservation,
}

impl ResolvedLocalSnapshot {
    pub(super) fn from_issued_parts(
        requested_root: PathBuf,
        canonical_live_root: PathBuf,
        snapshot_root: PathBuf,
        normalized: ResolvedSourceTree,
        resolution_observation: LocalSourceResolutionObservation,
    ) -> Self {
        Self {
            requested_root,
            canonical_live_root,
            snapshot_root,
            normalized,
            resolution_observation,
        }
    }

    pub fn requested_root(&self) -> &Path {
        &self.requested_root
    }

    pub fn canonical_live_root(&self) -> &Path {
        &self.canonical_live_root
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub const fn normalized(&self) -> &ResolvedSourceTree {
        &self.normalized
    }

    pub const fn resolution_observation(&self) -> &LocalSourceResolutionObservation {
        &self.resolution_observation
    }
}
