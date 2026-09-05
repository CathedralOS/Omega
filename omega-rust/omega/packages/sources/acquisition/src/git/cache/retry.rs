//! Retry incomplete acquisitions without treating transport failure as corruption.

use crate::SourceResolveError;
use crate::custody::tree::{
    CacheCustodyKind, git_cache_custody_byte_limit, verify_cache_custody_from_open_root,
};
use crate::limits::{GIT_CACHE_METADATA, LocalSourceLimits};
use crate::snapshot::permissions::make_open_tree_owner_writable;
use crate::tree::filesystem::io_error;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsStr;
use std::path::Path;

use super::custody::{open_retained_git_directory, verify_retained_git_directory_identity};
use super::invalidation::synchronize_cache_parent;

/// The caller holds the entry lock and permits a new acquisition. A missing
/// metadata record means no cached repository is usable; never reconstruct
/// that record over the existing objects. Remove only this bounded cache child
/// and let normal acquisition create and verify a fresh repository.
pub(crate) fn discard_incomplete_entry(
    cache_root: &Path,
    parent: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    limits: LocalSourceLimits,
) -> Result<bool, SourceResolveError> {
    let (entry, identity) = open_retained_git_directory(
        parent,
        entry_name,
        entry_root,
        "incomplete cache entry is not a concrete directory",
    )?;
    match entry.symlink_metadata(GIT_CACHE_METADATA) {
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&entry_root.join(GIT_CACHE_METADATA), error)),
    }
    verify_cache_custody_from_open_root(
        entry_root,
        entry
            .try_clone()
            .map_err(|error| io_error(entry_root, error))?,
        CacheCustodyKind::Git,
        git_cache_custody_byte_limit(limits),
    )?;
    verify_retained_git_directory_identity(
        parent,
        entry_name,
        &entry,
        &identity,
        entry_root,
        "incomplete cache entry changed before removal",
    )?;
    make_open_tree_owner_writable(&entry);
    entry
        .remove_open_dir_all()
        .map_err(|error| io_error(entry_root, error))?;
    synchronize_cache_parent(parent, cache_root)?;
    Ok(true)
}

/// Process failures may leave reusable objects behind, but only a fresh cache
/// verification can establish that. Object, tree, snapshot, and cleanup errors
/// deliberately do not qualify for this path.
pub(crate) fn may_preserve_cache(error: &SourceResolveError) -> bool {
    matches!(
        error,
        SourceResolveError::Git { .. }
            | SourceResolveError::GitTimedOut { .. }
            | SourceResolveError::GitOutputOverflow { .. }
            | SourceResolveError::GitResolutionTimedOut { .. }
            | SourceResolveError::GitResolutionCommandLimit { .. }
            | SourceResolveError::GitResolutionCapturedOutputLimit { .. }
    )
}
