//! Capability-relative filesystem primitives shared by source adapters.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir as CapabilityDirectory, OpenOptions as CapabilityOpenOptions},
};

use crate::SourceResolveError;

pub(crate) fn open_canonical_source_root(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let directory = open_absolute_directory_nofollow(canonical_root)
        .map_err(|error| io_error(canonical_root, error))?;
    let metadata = directory
        .dir_metadata()
        .map_err(|error| io_error(canonical_root, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: canonical_root.to_path_buf(),
        });
    }
    Ok(directory)
}

pub(crate) fn open_absolute_directory_nofollow(
    canonical_root: &Path,
) -> Result<CapabilityDirectory, std::io::Error> {
    use std::path::Component;

    let mut anchor = PathBuf::new();
    let mut relative_components = Vec::new();
    for component in canonical_root.components() {
        match component {
            Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
            Component::RootDir => anchor.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(name) => relative_components.push(name.to_os_string()),
            Component::ParentDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "canonical directory contains a parent component",
                ));
            }
        }
    }
    if anchor.as_os_str().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "canonical directory is not absolute",
        ));
    }

    let mut directory = CapabilityDirectory::open_ambient_dir(&anchor, ambient_authority())?;
    for component in relative_components {
        directory = directory.open_dir_nofollow(&component)?;
    }
    let metadata = directory.dir_metadata()?;
    if !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "opened path is not a directory",
        ));
    }
    Ok(directory)
}

pub(crate) fn open_captured_directory(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let child = directory
        .open_dir_nofollow(name)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = child
        .dir_metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    Ok(child)
}

pub(crate) fn read_capability_file_bounded(
    directory: &CapabilityDirectory,
    name: &OsStr,
    display_path: &Path,
    remaining: u64,
    limit: u64,
) -> Result<(Vec<u8>, bool), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = directory
        .open_with(name, &options)
        .map_err(|error| io_error(display_path, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !metadata.is_file() {
        return Err(SourceResolveError::UnsupportedFileType {
            path: display_path.to_path_buf(),
        });
    }
    if metadata.len() > remaining {
        return Err(SourceResolveError::TooManyBytes { limit });
    }

    let initial_capacity =
        usize::try_from(metadata.len()).map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|_| SourceResolveError::TooManyBytes { limit })?;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| io_error(display_path, error))?;
        if count == 0 {
            break;
        }
        let next_len = (bytes.len() as u64)
            .checked_add(count as u64)
            .ok_or(SourceResolveError::TooManyBytes { limit })?;
        if next_len > remaining {
            return Err(SourceResolveError::TooManyBytes { limit });
        }
        bytes.extend_from_slice(&chunk[..count]);
    }

    Ok((bytes, capability_metadata_is_executable(&metadata)))
}

#[cfg(unix)]
fn capability_metadata_is_executable(metadata: &cap_std::fs::Metadata) -> bool {
    use cap_fs_ext::OsMetadataExt;

    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn capability_metadata_is_executable(_metadata: &cap_std::fs::Metadata) -> bool {
    false
}

#[cfg(all(test, unix))]
pub(crate) fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
#[allow(dead_code)] // Kept for the existing source-internal cross-platform facade.
pub(crate) fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}

pub(crate) fn raw_os_bytes(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

pub(crate) fn io_error(path: &Path, error: std::io::Error) -> SourceResolveError {
    SourceResolveError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
