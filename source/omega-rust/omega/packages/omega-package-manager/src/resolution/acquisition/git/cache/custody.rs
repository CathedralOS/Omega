//! Retained directory identity and native Git repository-tree custody checks.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use cap_fs_ext::{DirExt, OpenOptionsFollowExt};

use crate::resolution::acquisition::{
    CACHE_CUSTODY_DEPTH_LIMIT, CACHE_CUSTODY_ENTRY_LIMIT, CacheCustodyKind, CapabilityDirectory,
    CapabilityMetadata, CapabilityOpenOptions, FollowSymlinks, SourceResolveError, io_error,
    open_cache_custody_directory, same_capability_file_identity,
};

use super::cache_invalid;

pub(super) fn open_retained_git_directory(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    message: &str,
) -> Result<(CapabilityDirectory, CapabilityMetadata), SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(path, message));
    }
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git directory changed between classification and no-follow open",
        ));
    }
    Ok((directory, opened))
}

pub(super) fn verify_retained_git_directory_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    retained: &CapabilityDirectory,
    expected: &CapabilityMetadata,
    path: &Path,
    message: &str,
) -> Result<(), SourceResolveError> {
    let named = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let opened = retained
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if named.file_type().is_symlink()
        || !named.is_dir()
        || !opened.is_dir()
        || !same_capability_file_identity(expected, &named)
        || !same_capability_file_identity(expected, &opened)
    {
        return Err(cache_invalid(path, message));
    }
    Ok(())
}

pub(super) fn verify_git_repository_tree_from_open_root(
    repository: &CapabilityDirectory,
    repository_path: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = repository
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    let mut pending = vec![(
        PathBuf::new(),
        repository_path.to_path_buf(),
        root_metadata,
        0usize,
    )];
    let mut observed = 0usize;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        observed = observed
            .checked_add(1)
            .ok_or_else(|| cache_invalid(&path, "Git repository entry count overflowed"))?;
        if observed > CACHE_CUSTODY_ENTRY_LIMIT {
            return Err(cache_invalid(
                repository_path,
                format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
            ));
        }
        let directory = open_cache_custody_directory(
            repository,
            &relative_path,
            &path,
            &classified,
            CacheCustodyKind::Git,
        )?;
        for child in directory
            .entries()
            .map_err(|error| io_error(&path, error))?
        {
            let child = child.map_err(|error| io_error(&path, error))?;
            let name = child.file_name();
            let child_path = path.join(&name);
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|error| io_error(&child_path, error))?;
            if metadata.file_type().is_symlink() {
                return Err(cache_invalid(
                    &child_path,
                    "symlinks are forbidden in the native Git repository",
                ));
            }
            if metadata.is_file() {
                verify_retained_git_regular_file(&directory, &name, &child_path, &metadata)?;
                observed = observed.checked_add(1).ok_or_else(|| {
                    cache_invalid(&child_path, "Git repository entry count overflowed")
                })?;
            } else if metadata.is_dir() {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| cache_invalid(&child_path, "Git repository depth overflowed"))?;
                if child_depth > CACHE_CUSTODY_DEPTH_LIMIT {
                    return Err(cache_invalid(
                        &child_path,
                        format!(
                            "Git repository exceeds its {CACHE_CUSTODY_DEPTH_LIMIT}-level depth ceiling"
                        ),
                    ));
                }
                pending.push((relative_path.join(&name), child_path, metadata, child_depth));
            } else {
                return Err(cache_invalid(
                    &child_path,
                    "native Git repository contains an unsupported filesystem entry kind",
                ));
            }
            if observed
                .checked_add(pending.len())
                .is_none_or(|total| total > CACHE_CUSTODY_ENTRY_LIMIT)
            {
                return Err(cache_invalid(
                    repository_path,
                    format!("Git repository exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry ceiling"),
                ));
            }
        }
    }
    Ok(())
}

fn verify_retained_git_regular_file(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .map_err(|error| cache_invalid(path, error.to_string()))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_invalid(
            path,
            "Git repository file changed between classification and no-follow open",
        ));
    }
    verify_git_regular_file_link_count(path, &opened)
}

#[cfg(unix)]
fn verify_git_regular_file_link_count(
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.nlink() != 1 {
        return Err(cache_invalid(
            path,
            "multiply-linked files are forbidden in the native Git repository",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_git_regular_file_link_count(
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

pub(super) fn reject_retained_git_path(
    root: &CapabilityDirectory,
    root_path: &Path,
    components: &[&str],
) -> Result<(), SourceResolveError> {
    let Some((leaf, parents)) = components.split_last() else {
        return Err(cache_invalid(root_path, "forbidden Git path is empty"));
    };
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(root_path, error))?;
    let mut path = root_path.to_path_buf();
    for parent in parents {
        path.push(parent);
        let metadata = match directory.symlink_metadata(parent) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error(&path, error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_invalid(
                &path,
                "cannot prove forbidden Git path absent beneath a non-directory",
            ));
        }
        let opened = directory
            .open_dir_nofollow(parent)
            .map_err(|error| cache_invalid(&path, error.to_string()))?;
        let opened_metadata = opened
            .dir_metadata()
            .map_err(|error| io_error(&path, error))?;
        if !same_capability_file_identity(&metadata, &opened_metadata) {
            return Err(cache_invalid(
                &path,
                "Git directory changed while checking forbidden indirection",
            ));
        }
        directory = opened;
    }
    path.push(leaf);
    match directory.symlink_metadata(leaf) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(&path, error)),
        Ok(_) => Err(cache_invalid(
            &path,
            "external Git object or directory indirection is forbidden",
        )),
    }
}
