//! Capability-relative invalidation of rejected Git cache entries.

use std::ffi::OsStr;
use std::path::Path;

use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;

use crate::SourceResolveError;
use crate::custody::platform::same_capability_file_identity;
#[cfg(all(test, unix))]
use crate::custody::publication::direct_cache_child_name;
#[cfg(all(test, unix))]
use crate::custody::tree::{CacheCustodyKind, verify_git_cache_root_custody};
use crate::limits::GIT_CACHE_METADATA;
use crate::tree::filesystem::io_error;
#[cfg(all(test, unix))]
use crate::tree::filesystem::open_absolute_directory_nofollow;

use crate::error::cache_invalid;

#[cfg(all(test, unix))]
pub(crate) fn invalidate_git_cache_entry_from_retained_parent(
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let cache_root = entry_root
        .parent()
        .ok_or_else(|| cache_invalid(entry_root, "Git cache entry has no cache parent"))?;
    verify_git_cache_root_custody(cache_root)?;
    let cache_directory = open_absolute_directory_nofollow(cache_root)
        .map_err(|error| cache_invalid(cache_root, error.to_string()))?;
    let entry_name = direct_cache_child_name(CacheCustodyKind::Git, cache_root, entry_root)?;
    invalidate_git_cache_entry_from_open_parent(
        cache_root,
        &cache_directory,
        entry_name,
        entry_root,
    )
}

pub(crate) fn invalidate_git_cache_entry_from_open_parent(
    cache_root: &Path,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
) -> Result<(), SourceResolveError> {
    let classified = cache_directory
        .symlink_metadata(entry_name)
        .map_err(|error| io_error(entry_root, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            entry_root,
            "Git cache invalidation target is not a concrete directory",
        ));
    }
    let entry_directory = cache_directory
        .open_dir_nofollow(entry_name)
        .map_err(|error| cache_invalid(entry_root, error.to_string()))?;
    let opened = entry_directory
        .dir_metadata()
        .map_err(|error| io_error(entry_root, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            entry_root,
            "Git cache entry changed while opening it for invalidation",
        ));
    }
    match entry_directory.remove_file(GIT_CACHE_METADATA) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&entry_root.join(GIT_CACHE_METADATA), error)),
    }
    synchronize_cache_parent(cache_directory, cache_root)
}

#[cfg(unix)]
pub(super) fn synchronize_cache_parent(
    cache_directory: &CapabilityDirectory,
    cache_root: &Path,
) -> Result<(), SourceResolveError> {
    cache_directory
        .try_clone()
        .map_err(|error| io_error(cache_root, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(cache_root, error))
}

#[cfg(not(unix))]
pub(super) fn synchronize_cache_parent(
    _cache_directory: &CapabilityDirectory,
    _cache_root: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}
