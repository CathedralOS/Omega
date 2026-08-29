//! Filesystem custody shared by local snapshots and Git cache entries.

use super::*;
pub(in crate::resolution::source) fn same_capability_file_identity(
    left: &CapabilityMetadata,
    right: &CapabilityMetadata,
) -> bool {
    use cap_fs_ext::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
pub(in crate::resolution::source) fn verify_capability_cache_node_owner_and_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    use cap_fs_ext::OsMetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != effective_user {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is not owned by the resolver's effective user",
        ));
    }
    if !metadata.file_type().is_symlink() && metadata.mode() & 0o022 != 0 {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is writable by group or other users",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(in crate::resolution::source) fn verify_capability_cache_node_owner_and_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn verify_cache_node_owner_and_mode(
    kind: CacheCustodyKind,
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    if metadata.uid() != effective_user {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is not owned by the resolver's effective user",
        ));
    }
    if !metadata.file_type().is_symlink() && metadata.mode() & 0o022 != 0 {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache entry is writable by group or other users",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_cache_node_owner_and_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    // Windows owner/DACL custody is verified through retained handles at each
    // concrete call site. Other non-Unix targets retain only the portable
    // kind and bounded-topology floor.
    Ok(())
}

#[cfg(windows)]
pub(in crate::resolution::source) fn verify_windows_open_cache_custody(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    verify_windows_open_cache_custody_with_owner_policy(
        kind,
        path,
        file,
        omega_platform_custody::WindowsFileOwnerPolicy::CurrentUserOnly,
    )
}

#[cfg(windows)]
fn verify_windows_open_cache_custody_with_owner_policy(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
    owner_policy: omega_platform_custody::WindowsFileOwnerPolicy,
) -> Result<(), SourceResolveError> {
    use omega_platform_custody::{WindowsFileCustodyViolation, inspect_open_windows_file_custody};

    let violation = inspect_open_windows_file_custody(file, owner_policy).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not inspect retained Windows cache custody: {error}"),
        )
    })?;
    if let Some(violation) = violation {
        let message = match violation {
            WindowsFileCustodyViolation::UntrustedOwner => {
                "cache entry is not owned by the resolver's current Windows user"
            }
            WindowsFileCustodyViolation::NullDacl => {
                "cache entry has a null DACL granting unrestricted access"
            }
            WindowsFileCustodyViolation::UntrustedMutationAuthority => {
                "cache entry grants mutation authority to an untrusted Windows principal"
            }
            WindowsFileCustodyViolation::UnsupportedAllowAce => {
                "cache entry contains an unsupported access-allowing Windows ACE"
            }
        };
        return Err(cache_custody_invalid(kind, path, message));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(in crate::resolution::source) fn verify_windows_open_cache_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _file: &File,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
pub(super) fn verify_windows_open_cache_directory_custody(
    kind: CacheCustodyKind,
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let directory = open_absolute_directory_nofollow(path).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain Windows cache custody directory: {error}"),
        )
    })?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody directory changed between classification and no-follow open",
        ));
    }
    verify_windows_open_cache_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )
}

#[cfg(windows)]
pub(super) fn verify_windows_open_cache_ancestry_custody(
    kind: CacheCustodyKind,
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let directory = open_absolute_directory_nofollow(path).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain Windows cache ancestry: {error}"),
        )
    })?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache ancestry changed between classification and no-follow open",
        ));
    }
    verify_windows_open_cache_custody_with_owner_policy(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
        omega_platform_custody::WindowsFileOwnerPolicy::CurrentUserSystemOrAdministrators,
    )
}

#[cfg(not(windows))]
pub(super) fn verify_windows_open_cache_directory_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
pub(super) fn verify_windows_open_cache_regular_file_custody(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    name: &OsStr,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain Windows cache file without following links: {error}"),
        )
    })?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache file changed between classification and no-follow open",
        ));
    }
    verify_windows_open_cache_custody(kind, path, &file.into_std())
}

#[cfg(not(windows))]
pub(super) fn verify_windows_open_cache_regular_file_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
pub(super) fn verify_windows_open_cache_link_custody(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    name: &OsStr,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain Windows cache reparse point: {error}"),
        )
    })?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache reparse point changed between classification and no-follow open",
        ));
    }
    verify_windows_open_cache_custody(kind, path, &file.into_std())
}

#[cfg(not(windows))]
pub(super) fn verify_windows_open_cache_link_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn verify_macos_cache_link_extended_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::extended_acl_has_allow_entry(
        path,
        omega_platform_custody::SymbolicLinkBehavior::InspectLink,
    )
    .map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not inspect cache symbolic-link extended ACL custody: {error}"),
        )
    })?;
    if has_allow_entry {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody contains an extended ACL allow entry",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_macos_cache_link_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(in crate::resolution::source) fn verify_macos_open_cache_extended_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let has_allow_entry = omega_platform_custody::open_file_extended_acl_has_allow_entry(file)
        .map_err(|error| {
            cache_custody_invalid(
                kind,
                path,
                format!("could not inspect retained cache extended ACL custody: {error}"),
            )
        })?;
    if has_allow_entry {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody contains an extended ACL allow entry",
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(in crate::resolution::source) fn verify_macos_open_cache_directory_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    let directory = open_absolute_directory_nofollow(path).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not retain cache custody directory: {error}"),
        )
    })?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache custody directory changed between classification and no-follow open",
        ));
    }
    verify_macos_open_cache_extended_acl_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )
}

#[cfg(not(target_os = "macos"))]
pub(in crate::resolution::source) fn verify_macos_open_cache_directory_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(in crate::resolution::source) fn verify_macos_open_cache_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _file: &File,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn verify_macos_open_cache_regular_file_acl_custody(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    name: &OsStr,
    classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let mut options = CapabilityOpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent.open_with(name, &options).map_err(|error| {
        cache_custody_invalid(
            kind,
            path,
            format!("could not open cache file without following links: {error}"),
        )
    })?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache file changed between classification and no-follow open",
        ));
    }
    verify_macos_open_cache_extended_acl_custody(kind, path, &file.into_std())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_macos_open_cache_regular_file_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn verify_macos_cache_link_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}
