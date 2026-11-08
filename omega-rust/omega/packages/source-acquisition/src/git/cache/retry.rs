// Cleanup of incomplete Git cache entries (`discard_incomplete_entry`) and
// classification of transient process failures (`may_preserve_cache`).
//
// When a caller holds the entry lock and finds an entry directory missing
// `GIT_CACHE_METADATA`, we never reconstruct metadata over existing objects; we
// verify custody of the incomplete directory, make it owner-writable, and
// remove it so normal acquisition creates a fresh repository. Process-level Git
// errors, timeouts, and output/launch budget errors (`may_preserve_cache`) do
// not invalidate an already-verified cache entry, whereas object, tree,
// snapshot, and cleanup errors do.

use crate::SourceResolveError;
use crate::custody::tree::{
    CacheCustodyKind, git_cache_custody_byte_limit, verify_cache_custody_from_open_root,
};
use crate::limits::{GIT_CACHE_METADATA, LocalSourceLimits};
use crate::snapshot::permissions::make_open_tree_owner_writable;
use crate::tree::filesystem::IoResultAt;
use crate::tree::filesystem::io_error;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsStr;
use std::path::Path;

use super::custody::{open_retained_git_directory, verify_retained_git_directory_identity};
use super::invalidation::synchronize_cache_parent;

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
        entry.try_clone().at(entry_root)?,
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
    entry.remove_open_dir_all().at(entry_root)?;
    synchronize_cache_parent(parent, cache_root)?;
    Ok(true)
}

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
