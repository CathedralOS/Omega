//! Platform custody checks for selected executable nodes.

use crate::SourceResolveError;
#[cfg(any(target_os = "macos", windows))]
use crate::custody::lock::same_std_and_capability_file_identity;
#[cfg(any(target_os = "macos", windows))]
use crate::tree::filesystem::open_absolute_directory_nofollow;
#[cfg(any(target_os = "macos", windows))]
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(any(target_os = "macos", windows))]
use cap_std::fs::OpenOptions as CapabilityOpenOptions;
#[cfg(target_os = "macos")]
use std::fs::File;
use std::path::Path;

pub(super) fn verify_git_transport_invocation_path(
    invocation_path: &Path,
    expected_canonical: &Path,
) -> Result<(), SourceResolveError> {
    let metadata = std::fs::symlink_metadata(invocation_path).map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: "transport invocation path is not a regular file or symbolic link".to_owned(),
        });
    }
    verify_git_transport_invocation_node_custody(invocation_path, &metadata)?;
    let canonical = invocation_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: invocation_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    if canonical != expected_canonical {
        return Err(SourceResolveError::GitExecutableChanged {
            path: invocation_path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn verify_git_transport_invocation_node_custody(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "transport invocation entry is owned by an unrelated user".to_owned(),
        });
    }
    if metadata.file_type().is_symlink() {
        verify_macos_path_extended_acl_custody(path, false)?;
    } else {
        verify_macos_open_executable_acl_custody(path, metadata)?;
    }
    Ok(())
}

#[cfg(windows)]
fn verify_git_transport_invocation_node_custody(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    verify_windows_executable_path_identity(path, metadata)
}

#[cfg(all(not(unix), not(windows)))]
fn verify_git_transport_invocation_node_custody(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_git_executable_custody(path: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
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
    if metadata.uid() != 0 && metadata.uid() != effective_user {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message:
                "resolver executable is owned by neither root nor the resolver's effective user"
                    .to_owned(),
        });
    }
    let mode = metadata.mode();
    if mode & 0o022 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable is writable by group or other users".to_owned(),
        });
    }
    if mode & 0o6000 != 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable must not carry set-user-ID or set-group-ID authority"
                .to_owned(),
        });
    }
    if mode & 0o111 == 0 {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no executable mode bit".to_owned(),
        });
    }
    verify_macos_open_executable_acl_custody(path, &metadata)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_path_extended_acl_custody(
    path: &Path,
    follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    let symbolic_link_behavior = if follow_symbolic_link {
        omega_platform_custody::SymbolicLinkBehavior::Follow
    } else {
        omega_platform_custody::SymbolicLinkBehavior::InspectLink
    };
    let has_allow_entry =
        omega_platform_custody::extended_acl_has_allow_entry(path, symbolic_link_behavior)
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "could not inspect resolver executable extended ACL custody: {error}"
                ),
            })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_macos_open_executable_acl_custody(
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let parent_path = path
        .parent()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no absolute custody parent".to_owned(),
        })?;
    let name = path
        .file_name()
        .ok_or_else(|| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable has no concrete filename".to_owned(),
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
            message: format!("could not open resolver executable without following links: {error}"),
        }
    })?;
    let opened = file
        .metadata()
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!("could not inspect retained resolver executable: {error}"),
        })?;
    if !opened.is_file() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(SourceResolveError::GitExecutableChanged {
            path: path.to_path_buf(),
        });
    }
    verify_macos_open_executable_extended_acl_custody(path, &file.into_std())
}

#[cfg(target_os = "macos")]
fn verify_macos_open_executable_extended_acl_custody(
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::open_file_extended_acl_has_allow_entry(file)
        .map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "could not inspect retained resolver executable extended ACL custody: {error}"
            ),
        })?;
    if has_allow_entry {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "resolver executable custody contains an extended ACL allow entry".to_owned(),
        });
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_path_extended_acl_custody(
    _path: &Path,
    _follow_symbolic_link: bool,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn verify_macos_open_executable_acl_custody(
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
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
