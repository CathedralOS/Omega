//! Resolver-neutral source-tree results and verified entries.

use std::path::PathBuf;

/// One completely traversed source tree and its canonical content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSourceTree {
    pub root: PathBuf,
    /// Number of file and symlink leaves. Directories participate in identity
    /// and limits but are not reported as files.
    pub file_count: usize,
    pub byte_count: u64,
    pub content_identity: String,
}

/// Compatibility name retained for callers written before source-tree custody
/// was separated from the local-source adapter.
pub type ResolvedLocalSource = ResolvedSourceTree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackageSourceEntry {
    pub relative_path: Vec<u8>,
    pub kind: VerifiedPackageSourceEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifiedPackageSourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}
