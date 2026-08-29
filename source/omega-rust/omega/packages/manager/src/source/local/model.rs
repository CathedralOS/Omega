//! Results and verified-entry data shared by local-source operations.

use std::path::{Path, PathBuf};

/// Compact canonical identity of one locally successful source resolution.
///
/// The resolver is the only issuer. The observation binds the caller's exact
/// request, its canonical live source, immutable publication, source limits,
/// snapshot custody, and the final exact-tree rehash. It records successful
/// non-admitting custody; it is not package admission.
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

    pub fn custody_identity(&self) -> &str {
        &self.custody_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSource {
    pub root: PathBuf,
    /// Number of file and symlink leaves. Directories participate in identity and limits but are
    /// not reported as files.
    pub file_count: usize,
    pub byte_count: u64,
    pub content_identity: String,
}

/// A resolver-owned immutable copy of a requested local source tree.
///
/// `requested_root` preserves the caller's locator, `canonical_live_root` identifies the mutable
/// tree that was captured, and `snapshot_root` is the only path downstream consumers should use.
/// `normalized` is re-resolved from that published snapshot rather than trusted from the live tree
/// or staging directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocalSnapshot {
    requested_root: PathBuf,
    canonical_live_root: PathBuf,
    snapshot_root: PathBuf,
    normalized: ResolvedLocalSource,
    resolution_observation: LocalSourceResolutionObservation,
}

impl ResolvedLocalSnapshot {
    pub(super) fn from_issued_parts(
        requested_root: PathBuf,
        canonical_live_root: PathBuf,
        snapshot_root: PathBuf,
        normalized: ResolvedLocalSource,
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

    pub const fn normalized(&self) -> &ResolvedLocalSource {
        &self.normalized
    }

    pub const fn resolution_observation(&self) -> &LocalSourceResolutionObservation {
        &self.resolution_observation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedPackageSourceEntry {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) kind: VerifiedPackageSourceEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VerifiedPackageSourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}
