//! Direct retained-cache custody for an accepted Git source.

use crate::custody::tree::{CacheCustodyMeasurement, git_cache_custody_byte_limit};
use crate::limits::{CACHE_CUSTODY_DEPTH_LIMIT, CACHE_CUSTODY_ENTRY_LIMIT, LocalSourceLimits};
use std::path::{Path, PathBuf};

/// Concrete post-helper retained storage accepted by the final capability-rooted
/// cache traversal. This is a measurement and its enforced limits, not a
/// process receipt or canonical identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRetainedStorageCustody {
    pub(crate) root: PathBuf,
    pub(crate) entry_limit: usize,
    pub(crate) byte_limit: u64,
    pub(crate) depth_limit: usize,
    pub(crate) entry_count: usize,
    pub(crate) logical_bytes: u64,
    pub(crate) maximum_depth: usize,
}

impl GitRetainedStorageCustody {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub const fn entry_limit(&self) -> usize {
        self.entry_limit
    }

    pub const fn byte_limit(&self) -> u64 {
        self.byte_limit
    }

    pub const fn depth_limit(&self) -> usize {
        self.depth_limit
    }

    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub const fn logical_bytes(&self) -> u64 {
        self.logical_bytes
    }

    pub const fn maximum_depth(&self) -> usize {
        self.maximum_depth
    }
}

pub(crate) fn git_retained_storage_custody(
    root: &Path,
    limits: LocalSourceLimits,
    measurement: CacheCustodyMeasurement,
) -> GitRetainedStorageCustody {
    GitRetainedStorageCustody {
        root: root.to_path_buf(),
        entry_limit: CACHE_CUSTODY_ENTRY_LIMIT,
        byte_limit: git_cache_custody_byte_limit(limits),
        depth_limit: CACHE_CUSTODY_DEPTH_LIMIT,
        entry_count: measurement.entry_count,
        logical_bytes: measurement.logical_bytes,
        maximum_depth: measurement.maximum_depth,
    }
}

pub(crate) fn validate_git_retained_storage_custody(
    custody: &GitRetainedStorageCustody,
    root: &Path,
    limits: LocalSourceLimits,
) -> bool {
    custody.root == root
        && custody.entry_limit == CACHE_CUSTODY_ENTRY_LIMIT
        && custody.byte_limit == git_cache_custody_byte_limit(limits)
        && custody.depth_limit == CACHE_CUSTODY_DEPTH_LIMIT
        && custody.entry_count > 0
        && custody.entry_count <= custody.entry_limit
        && custody.logical_bytes <= custody.byte_limit
        && custody.maximum_depth <= custody.depth_limit
}
