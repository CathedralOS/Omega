//! Platform custody checks for selected executable nodes.

#[cfg(windows)]
use crate::custody::lock::same_std_and_capability_file_identity;
#[cfg(windows)]
use crate::tree::filesystem::open_absolute_directory_nofollow;
use crate::SourceResolveError;
#[cfg(windows)]
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(windows)]
use cap_std::fs::OpenOptions as CapabilityOpenOptions;
use std::path::Path;

#[cfg(unix)]
pub(super) fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "canonical resolver executable is not a concrete regular file".to_owned(),
        });
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable is not directly launchable".to_owned(),
        });
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "canonical resolver executable is not a concrete regular file".to_owned(),
        });
    }
    verify_windows_executable_path_identity(path, &metadata)
}

#[cfg(windows)]
fn verify_windows_executable_path_identity(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let parent_path = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable entry has no absolute custody parent".to_owned(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable entry has no concrete filename".to_owned(),
        })?;
    let parent = open_absolute_directory_nofollow(parent_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: parent_path.to_path_buf(),
            message: format!("could not retain resolver executable parent: {error}"),
        }
    })?;
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "could not retain resolver executable entry without following reparse points: {error}"
            ),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not inspect retained resolver executable entry: {error}"),
        })?;
    if !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
pub(super) fn verify_git_executable_custody(_path: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}
