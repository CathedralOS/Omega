//! Capability-relative invalidation of rejected Git cache entries.

use std::ffi::OsStr;
use std::path::Path;

use cap_fs_ext::DirExt;

#[cfg(test)]
use crate::resolution::acquisition::{
    CacheCustodyKind, direct_cache_child_name, open_absolute_directory_nofollow,
    verify_git_cache_root_custody,
};
use crate::resolution::acquisition::{
    CapabilityDirectory, GIT_CACHE_METADATA, SourceResolveError, io_error,
    same_capability_file_identity,
};

use super::cache_invalid;

#[cfg(test)]
pub(in crate::resolution::acquisition) fn invalidate_git_cache_entry_from_retained_parent(
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

pub(in crate::resolution::acquisition) fn invalidate_git_cache_entry_from_open_parent(
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
    entry_directory
        .remove_file(GIT_CACHE_METADATA)
        .map_err(|error| io_error(&entry_root.join(GIT_CACHE_METADATA), error))?;
    cache_directory
        .try_clone()
        .map_err(|error| io_error(cache_root, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(cache_root, error))
}
