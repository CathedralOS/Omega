//! Sponsor paths, open descriptors and namespace snapshots.

use crate::checked_interpreter::filesystem_sponsor::accounts::DescriptorId;
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
    pub(crate) descriptor_id: DescriptorId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemSponsorSnapshot {
    pub entries: u64,
    pub total_logical_bytes: u64,
    pub unique_objects: u64,
    pub open_descriptors: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorNamespaceSnapshot {
    pub(crate) entries: Vec<FilesystemSponsorNamespaceEntry>,
    pub(crate) open_descriptors: u64,
    pub(crate) transaction_prepared: bool,
}

impl FilesystemSponsorNamespaceSnapshot {
    pub fn entries(&self) -> &[FilesystemSponsorNamespaceEntry] {
        &self.entries
    }

    pub const fn open_descriptors(&self) -> u64 {
        self.open_descriptors
    }

    pub const fn transaction_prepared(&self) -> bool {
        self.transaction_prepared
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSponsorNamespaceEntry {
    pub(crate) relative_path: PathBuf,
    pub(crate) kind: FilesystemSponsorNamespaceEntryKind,
}

impl FilesystemSponsorNamespaceEntry {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub const fn kind(&self) -> FilesystemSponsorNamespaceEntryKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorNamespaceEntryKind {
    Directory,
    Symlink { spelling_bytes: u64 },
    Object { group: u64, extent: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemSponsorEntry {
    Directory,
    Symlink {
        spelling_bytes: u64,
    },
    Object {
        extent: u64,
        names: u64,
        open_descriptors: u64,
    },
}
