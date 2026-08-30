//! Internal observations produced while traversing a source tree.

use std::path::PathBuf;

use crate::tree::ResolvedLocalSource;

#[derive(Debug)]
pub(super) struct SourceEntry {
    pub(super) relative_bytes: Vec<u8>,
    pub(super) relative_path: PathBuf,
    pub(super) kind: SourceEntryKind,
}

#[derive(Debug)]
pub(super) enum SourceEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug)]
pub(crate) struct CapturedLocalTree {
    pub(crate) normalized: ResolvedLocalSource,
    pub(crate) entries: Vec<CapturedLocalEntry>,
}

#[derive(Debug)]
pub(crate) struct CapturedLocalEntry {
    pub(crate) relative_path: PathBuf,
    pub(crate) relative_bytes: Vec<u8>,
    pub(crate) kind: CapturedLocalEntryKind,
}

#[derive(Debug)]
pub(crate) enum CapturedLocalEntryKind {
    Directory,
    File { bytes: Vec<u8>, executable: bool },
    Symlink { target_bytes: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTreePolicy {
    /// Mutable local package roots omit paths reserved for resolver or compiler output.
    LocalPackage,
    /// Resolver-owned materializations must be hashed exactly as published.
    ExactMaterialized,
}
