//! Results and verified-entry data shared by local-source operations.

use std::path::PathBuf;

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
    pub requested_root: PathBuf,
    pub canonical_live_root: PathBuf,
    pub snapshot_root: PathBuf,
    pub normalized: ResolvedLocalSource,
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
