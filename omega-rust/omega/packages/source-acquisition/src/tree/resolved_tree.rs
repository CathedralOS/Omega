// Resolver-neutral source-tree results (`ResolvedSourceTree` /
// `ResolvedLocalSource`) and verified package source entries
// (`VerifiedPackageSourceEntry`).

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ResolvedSourceTree {
    pub root: PathBuf,
    pub file_count: usize, // File and symlink leaves (directories participate in identity/limits only).
    pub byte_count: u64,
    pub content_identity: String,
}

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
