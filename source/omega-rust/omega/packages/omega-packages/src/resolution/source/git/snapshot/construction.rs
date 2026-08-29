//! Capability-relative snapshot directory, file, and symlink construction.

#[cfg(not(unix))]
use crate::resolution::source::git_tree_invalid;
use crate::resolution::source::{
    CacheCustodyKind, SourceResolveError, cache_custody_invalid, io_error,
};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt as CapabilityOpenOptionsExt;
use cap_std::fs::{Dir as CapabilityDirectory, OpenOptions as CapabilityOpenOptions};
#[cfg(unix)]
use std::ffi::OsString;
use std::io::Write;
use std::path::{Component, Path};

pub(in crate::resolution::source) fn open_or_create_snapshot_directory(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(display_root, error))?;
    let mut display_path = display_root.to_path_buf();
    for component in relative_path.components() {
        let Component::Normal(name) = component else {
            return Err(cache_custody_invalid(
                kind,
                &display_path,
                "snapshot materialization received a noncanonical relative directory",
            ));
        };
        display_path.push(name);
        match directory.create_dir(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&display_path, error)),
        }
        directory = directory.open_dir_nofollow(name).map_err(|error| {
            cache_custody_invalid(
                kind,
                &display_path,
                format!("snapshot directory is not a stable concrete child: {error}"),
            )
        })?;
    }
    Ok(directory)
}

pub(in crate::resolution::source) fn write_snapshot_file_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), SourceResolveError> {
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot file has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    let mut options = CapabilityOpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = parent.open_with(name, &options).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(
                kind,
                &display_path,
                "snapshot file destination already exists",
            )
        } else {
            io_error(&display_path, error)
        }
    })?;
    file.write_all(bytes)
        .map_err(|error| io_error(&display_path, error))?;
    file.sync_all()
        .map_err(|error| io_error(&display_path, error))?;
    set_open_snapshot_file_mode(&file, &display_path, executable)
}

#[cfg(unix)]
pub(in crate::resolution::source) fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    use std::os::unix::ffi::OsStringExt;

    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent
        .symlink_contents(OsString::from_vec(target.to_vec()), name)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
            } else {
                io_error(&display_path, error)
            }
        })
}

#[cfg(not(unix))]
pub(in crate::resolution::source) fn create_snapshot_symlink_from_open_root(
    kind: CacheCustodyKind,
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_root: &Path,
    target: &[u8],
) -> Result<(), SourceResolveError> {
    let target = std::str::from_utf8(target).map_err(|_| {
        git_tree_invalid(target, "symlink target cannot be represented on this host")
    })?;
    let parent_path = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let parent = open_or_create_snapshot_directory(kind, root, parent_path, display_root)?;
    let name = relative_path.file_name().ok_or_else(|| {
        cache_custody_invalid(
            kind,
            &display_root.join(relative_path),
            "snapshot symlink has no relative name",
        )
    })?;
    let display_path = display_root.join(relative_path);
    parent.symlink_file(target, name).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            cache_custody_invalid(kind, &display_path, "snapshot symlink already exists")
        } else {
            io_error(&display_path, error)
        }
    })
}

#[cfg(unix)]
pub(super) fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    executable: bool,
) -> Result<(), SourceResolveError> {
    use cap_std::fs::PermissionsExt;

    let mode = if executable { 0o555 } else { 0o444 };
    file.set_permissions(cap_std::fs::Permissions::from_mode(mode))
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
pub(super) fn set_open_snapshot_file_mode(
    file: &cap_std::fs::File,
    path: &Path,
    _executable: bool,
) -> Result<(), SourceResolveError> {
    let mut permissions = file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions)
        .map_err(|error| io_error(path, error))
}
