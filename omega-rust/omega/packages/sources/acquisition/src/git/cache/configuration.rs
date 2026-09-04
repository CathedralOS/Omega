//! Atomic restoration of the compiler-owned Git configuration.

use crate::custody::platform::same_capability_file_identity;
use crate::error::SourceResolveError;
use crate::limits::GIT_CONFIG_SHA256;
use crate::tree::filesystem::io_error;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use omega_platform_custody::record_file::{RecordFileLimits, RecordFileRoot};
use std::ffi::OsStr;
use std::path::Path;

use crate::error::cache_invalid;

pub(crate) fn replace_canonical_git_control_file(
    entry: &CapabilityDirectory,
    repository_name: &OsStr,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let classified = entry
        .symlink_metadata(repository_name)
        .map_err(|error| io_error(repository_path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            repository_path,
            "Git repository is not a concrete directory",
        ));
    }
    let directory = entry
        .open_dir_nofollow(repository_name)
        .map_err(|error| cache_invalid(repository_path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            repository_path,
            "Git repository changed while opening it for configuration replacement",
        ));
    }
    replace_canonical_git_control_file_from_open_repository(
        &directory,
        repository_path,
        canonical_config,
    )
}

pub(crate) fn replace_canonical_git_control_file_from_open_repository(
    repository: &CapabilityDirectory,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let config_path = repository_path.join("config");
    let directory = repository
        .try_clone()
        .map_err(|error| io_error(repository_path, error))?;
    let root = RecordFileRoot::from_directory(directory, repository_path.to_path_buf()).map_err(
        |error| {
            cache_invalid(
                repository_path,
                format!("failed to bind Git configuration directory custody: {error:?}"),
            )
        },
    )?;
    root.replace_existing(
        Path::new("config"),
        canonical_config,
        RecordFileLimits {
            maximum_bytes: GIT_CONFIG_SHA256.len(),
        },
    )
    .map_err(|error| {
        cache_invalid(
            &config_path,
            format!("failed to atomically restore canonical Git configuration: {error:?}"),
        )
    })
}
