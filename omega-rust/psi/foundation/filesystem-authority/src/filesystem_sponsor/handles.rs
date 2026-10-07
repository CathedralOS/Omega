//! Account-bound paths and live writer IDs; neither describes a namespace.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorPath {
    pub(crate) account_id: u64,
    pub(crate) relative: PathBuf,
}
impl FilesystemSponsorPath {
    pub fn relative(&self) -> &Path {
        &self.relative
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOpenDescriptor {
    pub(crate) account_id: u64,
    pub(crate) writer_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorEntry {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object { extent: u64 },
}
