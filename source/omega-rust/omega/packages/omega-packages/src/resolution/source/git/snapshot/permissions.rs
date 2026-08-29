//! Canonical snapshot modes, immutable-tree verification, and cleanup permissions.

#[cfg(unix)]
use crate::resolution::source::CANONICAL_DIRECTORY_MODE;
#[cfg(test)]
use crate::resolution::source::is_executable;
use crate::resolution::source::{
    CacheCustodyKind, SourceResolveError, cache_custody_invalid, io_error,
    same_capability_file_identity,
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};
use std::path::Path;

use super::construction::set_open_snapshot_file_mode;

pub(in crate::resolution::source) fn verify_open_snapshot_tree_modes(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let root_metadata = root
        .dir_metadata()
        .map_err(|error| io_error(display_root, error))?;
    verify_capability_snapshot_directory_mode(kind, display_root, &root_metadata)?;
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during verification: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during verification",
                ));
            }
            verify_open_snapshot_tree_modes(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during verification: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during verification",
                ));
            }
            verify_capability_snapshot_file_mode(kind, &path, &opened)?;
        } else if !metadata.file_type().is_symlink() {
            return Err(cache_custody_invalid(
                kind,
                &path,
                "snapshot contains an unsupported filesystem entry type",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_directory_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if metadata.mode() & 0o7777 != u32::from(CANONICAL_DIRECTORY_MODE) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot directory mode is not canonical 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_directory_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    if !matches!(metadata.mode() & 0o7777, 0o444 | 0o555) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file mode is not canonical 0444 or 0555",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_capability_snapshot_file_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    if !metadata.permissions().readonly() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "snapshot file is writable",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn make_open_snapshot_read_only(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    display_root: &Path,
) -> Result<(), SourceResolveError> {
    let entries = root
        .entries()
        .map_err(|error| io_error(display_root, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(display_root, error))?;
        let name = entry.file_name();
        let path = display_root.join(&name);
        let metadata = root
            .symlink_metadata(&name)
            .map_err(|error| io_error(&path, error))?;
        if metadata.is_dir() {
            let directory = root.open_dir_nofollow(&name).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot directory changed during finalization: {error}"),
                )
            })?;
            let opened = directory
                .dir_metadata()
                .map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot directory changed during read-only finalization",
                ));
            }
            make_open_snapshot_read_only(kind, &directory, &path)?;
        } else if metadata.is_file() {
            let mut options = CapabilityOpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = root.open_with(&name, &options).map_err(|error| {
                cache_custody_invalid(
                    kind,
                    &path,
                    format!("snapshot file changed during finalization: {error}"),
                )
            })?;
            let opened = file.metadata().map_err(|error| io_error(&path, error))?;
            if !same_capability_file_identity(&metadata, &opened) {
                return Err(cache_custody_invalid(
                    kind,
                    &path,
                    "snapshot file changed during read-only finalization",
                ));
            }
            set_open_snapshot_file_mode(&file, &path, capability_is_executable(&metadata))?;
        }
    }
    set_open_snapshot_directory_read_only(root, display_root)
}

#[cfg(unix)]
fn capability_is_executable(metadata: &CapabilityMetadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_is_executable(_metadata: &CapabilityMetadata) -> bool {
    false
}

#[cfg(unix)]
fn set_open_snapshot_directory_read_only(
    directory: &CapabilityDirectory,
    path: &Path,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    directory
        .try_clone()
        .map_err(|error| io_error(path, error))?
        .into_std_file()
        .set_permissions(std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn set_open_snapshot_directory_read_only(
    _directory: &CapabilityDirectory,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_file_mode(path: &Path, executable: bool) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_file_mode(path: &Path, _executable: bool) -> Result<(), SourceResolveError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|error| io_error(path, error))
}

#[cfg(test)]
pub(in crate::resolution::source) fn make_snapshot_read_only(
    root: &Path,
) -> Result<(), SourceResolveError> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        for entry in std::fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            let path = entry.path();
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            if metadata.is_dir() {
                directories.push(path);
            } else if metadata.is_file() {
                set_snapshot_file_mode(&path, is_executable(&metadata))?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        set_snapshot_directory_read_only(&directory)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
fn set_snapshot_directory_read_only(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555))
        .map_err(|error| io_error(path, error))
}

#[cfg(all(test, not(unix)))]
fn set_snapshot_directory_read_only(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

pub(in crate::resolution::source) fn make_open_tree_owner_writable(root: &CapabilityDirectory) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(directory) = root.try_clone() {
            let _ = directory
                .into_std_file()
                .set_permissions(std::fs::Permissions::from_mode(0o700));
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name)
                    && metadata.is_dir()
                    && let Ok(directory) = root.open_dir_nofollow(&name)
                {
                    make_open_tree_owner_writable(&directory);
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        if let Ok(directory) = root.try_clone() {
            let directory = directory.into_std_file();
            if let Ok(metadata) = directory.metadata() {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                let _ = directory.set_permissions(permissions);
            }
        }
        if let Ok(entries) = root.entries() {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Ok(metadata) = root.symlink_metadata(&name) {
                    if metadata.is_dir() {
                        if let Ok(directory) = root.open_dir_nofollow(&name) {
                            make_open_tree_owner_writable(&directory);
                        }
                    } else if metadata.is_file() {
                        let mut options = CapabilityOpenOptions::new();
                        options.read(true).follow(FollowSymlinks::No);
                        if let Ok(file) = root.open_with(&name, &options)
                            && let Ok(metadata) = file.metadata()
                        {
                            let mut permissions = metadata.permissions();
                            permissions.set_readonly(false);
                            let _ = file.set_permissions(permissions);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
pub(in crate::resolution::source) fn make_tree_owner_writable(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(metadata) = std::fs::symlink_metadata(root)
            && metadata.is_dir()
        {
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Ok(metadata) = std::fs::symlink_metadata(&path)
                        && metadata.is_dir()
                    {
                        make_tree_owner_writable(&path);
                    }
                }
            }
        }
    }
}
