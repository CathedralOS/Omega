//! Filesystem custody shared by local snapshots and Git cache entries.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(in crate::resolution::source) enum CacheCustodyKind {
    Git,
    LocalSnapshot,
}

pub(in crate::resolution::source) fn read_bounded_cache_record(
    kind: CacheCustodyKind,
    root: &Path,
    relative_path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    verify_cache_custody_root(root, kind)?;
    let directory = open_absolute_directory_nofollow(root)
        .map_err(|error| cache_custody_invalid(kind, root, error.to_string()))?;
    read_bounded_cache_record_from_open_directory(
        kind,
        &directory,
        root,
        relative_path,
        maximum_bytes,
    )
}

pub(in crate::resolution::source) fn read_bounded_cache_record_from_open_directory(
    kind: CacheCustodyKind,
    directory: &CapabilityDirectory,
    root: &Path,
    relative_path: &Path,
    maximum_bytes: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    let directory = directory
        .try_clone()
        .map_err(|error| io_error(root, error))?;
    let record_root =
        RecordFileRoot::from_directory(directory, root.to_path_buf()).map_err(|error| {
            cache_custody_invalid(
                kind,
                root,
                format!("failed to retain cache record directory: {error:?}"),
            )
        })?;
    let record = record_root
        .read(relative_path, RecordFileLimits { maximum_bytes })
        .map_err(|error| {
            cache_custody_invalid(
                kind,
                &root.join(relative_path),
                format!("failed to read bounded cache record: {error:?}"),
            )
        })?;
    Ok(record.bytes().to_vec())
}

pub(in crate::resolution::source) fn verify_git_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::Git,
        git_cache_custody_byte_limit(limits),
    )
}

pub(in crate::resolution::source) fn verify_git_cache_root_custody(
    root: &Path,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::Git)
}

pub(in crate::resolution::source) fn verify_local_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::LocalSnapshot,
        local_cache_custody_byte_limit(limits),
    )
}

pub(in crate::resolution::source) fn verify_local_cache_root_custody(
    root: &Path,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::LocalSnapshot)
}

pub(in crate::resolution::source) fn verify_cache_custody_root(
    root: &Path,
    kind: CacheCustodyKind,
) -> Result<(), SourceResolveError> {
    verify_cache_ancestry(kind, root)?;
    let metadata = std::fs::symlink_metadata(root).map_err(|error| io_error(root, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            root,
            "cache custody root is not a concrete directory",
        ));
    }
    verify_cache_node_owner_and_mode(kind, root, &metadata)?;
    verify_macos_open_cache_directory_acl_custody(kind, root, &metadata)?;
    verify_windows_open_cache_directory_custody(kind, root, &metadata)
}

#[cfg(unix)]
fn verify_cache_ancestry(kind: CacheCustodyKind, root: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    for ancestor in root.ancestors() {
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|error| io_error(ancestor, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry contains a non-directory or symlink",
            ));
        }
        if metadata.uid() != effective_user && metadata.uid() != 0 {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry is owned by an unrelated user",
            ));
        }
        let mode = metadata.mode();
        if mode & 0o022 != 0 && mode & 0o1000 == 0 {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry is externally writable without sticky-entry protection",
            ));
        }
        verify_macos_open_cache_directory_acl_custody(kind, ancestor, &metadata)?;
    }
    Ok(())
}

#[cfg(windows)]
fn verify_cache_ancestry(kind: CacheCustodyKind, root: &Path) -> Result<(), SourceResolveError> {
    for ancestor in root.ancestors() {
        let metadata =
            std::fs::symlink_metadata(ancestor).map_err(|error| io_error(ancestor, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(cache_custody_invalid(
                kind,
                ancestor,
                "cache custody ancestry contains a non-directory or reparse point",
            ));
        }
        verify_windows_open_cache_ancestry_custody(kind, ancestor, &metadata)?;
    }
    Ok(())
}

#[cfg(all(not(unix), not(windows)))]
fn verify_cache_ancestry(_kind: CacheCustodyKind, _root: &Path) -> Result<(), SourceResolveError> {
    Ok(())
}

pub(in crate::resolution::source) fn git_cache_custody_byte_limit(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_mul(3)
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

pub(in crate::resolution::source) fn local_cache_custody_byte_limit(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

pub(in crate::resolution::source) fn verify_cache_custody(
    root: &Path,
    kind: CacheCustodyKind,
    byte_limit: u64,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, kind)?;
    let root_directory = open_absolute_directory_nofollow(root)
        .map_err(|error| cache_custody_invalid(kind, root, error.to_string()))?;
    verify_cache_custody_from_open_root(root, root_directory, kind, byte_limit)
}

pub(in crate::resolution::source) fn verify_cache_custody_from_open_root(
    root: &Path,
    root_directory: CapabilityDirectory,
    kind: CacheCustodyKind,
    byte_limit: u64,
) -> Result<(), SourceResolveError> {
    let root_metadata = root_directory
        .dir_metadata()
        .map_err(|error| io_error(root, error))?;
    let mut pending = vec![(PathBuf::new(), root.to_path_buf(), root_metadata, 0usize)];
    let mut observed = 0usize;
    let mut logical_bytes = 0u64;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        observed = observed.checked_add(1).ok_or_else(|| {
            cache_custody_invalid(kind, &path, "cache custody entry count overflowed")
        })?;
        if observed > CACHE_CUSTODY_ENTRY_LIMIT {
            return Err(cache_custody_invalid(
                kind,
                root,
                format!(
                    "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                ),
            ));
        }
        let directory = open_cache_custody_directory(
            &root_directory,
            &relative_path,
            &path,
            &classified,
            kind,
        )?;
        let metadata = directory
            .dir_metadata()
            .map_err(|error| io_error(&path, error))?;
        verify_capability_cache_node_owner_and_mode(kind, &path, &metadata)?;
        let directory_file = directory
            .try_clone()
            .map_err(|error| io_error(&path, error))?
            .into_std_file();
        verify_macos_open_cache_extended_acl_custody(kind, &path, &directory_file)?;
        verify_windows_open_cache_custody(kind, &path, &directory_file)?;

        let children = directory
            .entries()
            .map_err(|error| io_error(&path, error))?;
        for child in children {
            let child = child.map_err(|error| io_error(&path, error))?;
            let name = child.file_name();
            let child_path = path.join(&name);
            if !cache_custody_has_capacity(observed, pending.len()) {
                return Err(cache_custody_invalid(
                    kind,
                    root,
                    format!(
                        "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                    ),
                ));
            }
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|error| io_error(&child_path, error))?;
            verify_capability_cache_node_owner_and_mode(kind, &child_path, &metadata)?;
            let file_type = metadata.file_type();
            if file_type.is_file() {
                verify_macos_open_cache_regular_file_acl_custody(
                    kind,
                    &child_path,
                    &directory,
                    &name,
                    &metadata,
                )?;
                verify_windows_open_cache_regular_file_custody(
                    kind,
                    &child_path,
                    &directory,
                    &name,
                    &metadata,
                )?;
            } else if file_type.is_symlink() {
                verify_macos_cache_link_extended_acl_custody(kind, &child_path)?;
                verify_windows_open_cache_link_custody(
                    kind,
                    &child_path,
                    &directory,
                    &name,
                    &metadata,
                )?;
            }
            if file_type.is_file() || file_type.is_symlink() {
                logical_bytes = logical_bytes
                    .checked_add(metadata.len())
                    .filter(|bytes| *bytes <= byte_limit)
                    .ok_or_else(|| {
                        cache_custody_invalid(
                            kind,
                            root,
                            format!(
                                "cache custody tree exceeds its {byte_limit}-byte logical resident ceiling"
                            ),
                        )
                    })?;
                observed = observed.checked_add(1).ok_or_else(|| {
                    cache_custody_invalid(kind, &child_path, "cache custody entry count overflowed")
                })?;
            } else if file_type.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    cache_custody_invalid(kind, &child_path, "cache custody depth overflowed")
                })?;
                if child_depth > CACHE_CUSTODY_DEPTH_LIMIT {
                    return Err(cache_custody_invalid(
                        kind,
                        &child_path,
                        format!(
                            "cache custody tree exceeds its {CACHE_CUSTODY_DEPTH_LIMIT}-level depth ceiling"
                        ),
                    ));
                }
                pending.push((relative_path.join(&name), child_path, metadata, child_depth));
            } else {
                return Err(cache_custody_invalid(
                    kind,
                    &child_path,
                    "cache custody contains an unsupported filesystem entry kind",
                ));
            }
            if observed > CACHE_CUSTODY_ENTRY_LIMIT {
                // The retained-entry check above should make this unreachable, but keep the
                // ceiling explicit if traversal accounting changes.
                return Err(cache_custody_invalid(
                    kind,
                    root,
                    format!(
                        "cache custody tree exceeds its {CACHE_CUSTODY_ENTRY_LIMIT}-entry metadata ceiling"
                    ),
                ));
            }
        }
    }
    Ok(())
}

pub(in crate::resolution::source) fn cache_custody_has_capacity(
    observed: usize,
    pending: usize,
) -> bool {
    observed
        .checked_add(pending)
        .is_some_and(|retained| retained < CACHE_CUSTODY_ENTRY_LIMIT)
}

#[cfg(test)]
pub(in crate::resolution::source) fn publish_cache_directory(
    kind: CacheCustodyKind,
    parent: &Path,
    staged: &Path,
    publication: &Path,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent, kind)?;
    let directory = open_absolute_directory_nofollow(parent)
        .map_err(|error| cache_custody_invalid(kind, parent, error.to_string()))?;
    let staged_name = direct_cache_child_name(kind, parent, staged)?;
    let publication_name = direct_cache_child_name(kind, parent, publication)?;
    publish_cache_directory_from_open_parent(
        kind,
        parent,
        &directory,
        staged_name,
        publication_name,
        None,
    )
}

pub(in crate::resolution::source) fn direct_cache_child_name<'a>(
    kind: CacheCustodyKind,
    parent: &Path,
    child: &'a Path,
) -> Result<&'a OsStr, SourceResolveError> {
    let relative = child.strip_prefix(parent).map_err(|_| {
        cache_custody_invalid(
            kind,
            child,
            "cache publication is outside its retained parent",
        )
    })?;
    let mut components = relative.components();
    let name = match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(name)), None) => name,
        _ => {
            return Err(cache_custody_invalid(
                kind,
                child,
                "cache publication is not a direct child of its retained parent",
            ));
        }
    };
    Ok(name)
}

pub(in crate::resolution::source) fn retained_cache_directory_exists(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<bool, SourceResolveError> {
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
            cache_custody_invalid(kind, path, "cache entry is not a concrete directory"),
        ),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

pub(in crate::resolution::source) fn publish_cache_directory_from_open_parent(
    kind: CacheCustodyKind,
    parent: &Path,
    directory: &CapabilityDirectory,
    staged_name: &OsStr,
    publication_name: &OsStr,
    expected_staged: Option<&CapabilityMetadata>,
) -> Result<(), SourceResolveError> {
    let staged_path = parent.join(staged_name);
    let publication_path = parent.join(publication_name);
    let staged_metadata = directory
        .symlink_metadata(staged_name)
        .map_err(|error| io_error(&staged_path, error))?;
    if staged_metadata.file_type().is_symlink() || !staged_metadata.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage is not a concrete directory",
        ));
    }
    if expected_staged
        .is_some_and(|expected| !same_capability_file_identity(expected, &staged_metadata))
    {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage no longer identifies the retained directory",
        ));
    }
    match directory.symlink_metadata(publication_name) {
        Ok(_) => {
            return Err(cache_custody_invalid(
                kind,
                &publication_path,
                "cache publication destination already exists",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&publication_path, error)),
    }

    directory
        .rename(staged_name, directory, publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    let published_metadata = directory
        .symlink_metadata(publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    if !published_metadata.is_dir()
        || !same_capability_file_identity(&staged_metadata, &published_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            &publication_path,
            "published cache directory does not identify the staged directory",
        ));
    }
    directory
        .try_clone()
        .map_err(|error| io_error(parent, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(parent, error))?;
    Ok(())
}

pub(in crate::resolution::source) fn open_cache_custody_directory(
    root: &CapabilityDirectory,
    relative_path: &Path,
    display_path: &Path,
    classified: &CapabilityMetadata,
    kind: CacheCustodyKind,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let mut directory = root
        .try_clone()
        .map_err(|error| io_error(display_path, error))?;
    for component in relative_path.components() {
        use std::path::Component;

        let Component::Normal(name) = component else {
            return Err(cache_custody_invalid(
                kind,
                display_path,
                "cache custody queued a noncanonical relative directory path",
            ));
        };
        directory = directory
            .open_dir_nofollow(name)
            .map_err(|error| cache_custody_invalid(kind, display_path, error.to_string()))?;
    }
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(display_path, error))?;
    if !opened.is_dir() || !same_capability_file_identity(classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            display_path,
            "cache directory changed between classification and no-follow open",
        ));
    }
    Ok(directory)
}

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
fn verify_cache_node_owner_and_mode(
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
fn verify_cache_node_owner_and_mode(
    _kind: CacheCustodyKind,
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    // Windows owner/DACL custody is verified through retained handles at each
    // concrete call site. Other non-Unix targets retain only the portable
    // kind and bounded-topology floor.
    Ok(())
}

pub(in crate::resolution::source) fn cache_custody_invalid(
    kind: CacheCustodyKind,
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    match kind {
        CacheCustodyKind::Git => cache_invalid(path, message),
        CacheCustodyKind::LocalSnapshot => local_snapshot_invalid(path, message),
    }
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
fn verify_windows_open_cache_directory_custody(
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
fn verify_windows_open_cache_ancestry_custody(
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
fn verify_windows_open_cache_directory_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _classified: &std::fs::Metadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
fn verify_windows_open_cache_regular_file_custody(
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
fn verify_windows_open_cache_regular_file_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(windows)]
fn verify_windows_open_cache_link_custody(
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
fn verify_windows_open_cache_link_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_macos_cache_link_extended_acl_custody(
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
fn verify_macos_cache_link_extended_acl_custody(
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
fn verify_macos_open_cache_regular_file_acl_custody(
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
fn verify_macos_open_cache_regular_file_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
    _parent: &CapabilityDirectory,
    _name: &OsStr,
    _classified: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn verify_macos_cache_link_extended_acl_custody(
    _kind: CacheCustodyKind,
    _path: &Path,
) -> Result<(), SourceResolveError> {
    Ok(())
}

pub(in crate::resolution::source) struct CacheEntryLock {
    pub(in crate::resolution::source) file: File,
    pub(in crate::resolution::source) parent: CapabilityDirectory,
    pub(in crate::resolution::source) kind: CacheCustodyKind,
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) lock_name: OsString,
}

impl CacheEntryLock {
    pub(in crate::resolution::source) fn open_retained(
        kind: CacheCustodyKind,
        path: &Path,
    ) -> Result<(File, CapabilityDirectory, OsString), SourceResolveError> {
        let parent_path = path.parent().ok_or_else(|| {
            cache_custody_invalid(kind, path, "cache lock has no publication parent")
        })?;
        verify_cache_custody_root(parent_path, kind)?;
        let parent = open_absolute_directory_nofollow(parent_path)
            .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
        let lock_name = direct_cache_child_name(kind, parent_path, path)?.to_os_string();
        let mut options = CapabilityOpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.mode(0o600);
        let capability_file = parent.open_with(&lock_name, &options).map_err(|error| {
            cache_custody_invalid(
                kind,
                path,
                format!("could not open cache lock without following links: {error}"),
            )
        })?;
        let handle_metadata = capability_file
            .metadata()
            .map_err(|error| io_error(path, error))?;
        let path_metadata = parent
            .symlink_metadata(&lock_name)
            .map_err(|error| io_error(path, error))?;
        if !handle_metadata.is_file()
            || path_metadata.file_type().is_symlink()
            || !path_metadata.is_file()
            || !same_capability_file_identity(&handle_metadata, &path_metadata)
        {
            return Err(cache_custody_invalid(
                kind,
                path,
                "cache lock is not a stable regular file beneath its retained parent",
            ));
        }
        verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
        let file = capability_file.into_std();
        verify_macos_open_cache_extended_acl_custody(kind, path, &file)?;
        verify_windows_open_cache_custody(kind, path, &file)?;
        Ok((file, parent, lock_name))
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn open_git(path: &Path) -> Result<File, SourceResolveError> {
        let (file, _, _) = Self::open_retained(CacheCustodyKind::Git, path)?;
        Ok(file)
    }

    pub(in crate::resolution::source) fn acquire_with_git_budget(
        path: &Path,
        executor: &GitExecutor,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        loop {
            executor.verify_budget()?;
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let remaining = executor.remaining_time()?;
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if let Err(error) = executor.verify_budget() {
            let _ = file.unlock();
            return Err(error);
        }
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    #[cfg(test)]
    pub(in crate::resolution::source) fn acquire(path: &Path) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::Git, path)?;
        file.lock().map_err(|error| io_error(path, error))?;
        verify_cache_lock_path_identity(CacheCustodyKind::Git, path, &parent, &lock_name, &file)?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::Git,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    pub(in crate::resolution::source) fn acquire_local(
        path: &Path,
    ) -> Result<Self, SourceResolveError> {
        Self::acquire_local_with_timeout(path, LOCAL_SNAPSHOT_LOCK_TIMEOUT)
    }

    pub(in crate::resolution::source) fn acquire_local_with_timeout(
        path: &Path,
        timeout: Duration,
    ) -> Result<Self, SourceResolveError> {
        let (file, parent, lock_name) = Self::open_retained(CacheCustodyKind::LocalSnapshot, path)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    };
                    if remaining.is_zero() {
                        return Err(local_snapshot_lock_timed_out(path, timeout));
                    }
                    std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(io_error(path, error));
                }
            }
        }
        if started.elapsed() >= timeout {
            let _ = file.unlock();
            return Err(local_snapshot_lock_timed_out(path, timeout));
        }
        verify_cache_lock_path_identity(
            CacheCustodyKind::LocalSnapshot,
            path,
            &parent,
            &lock_name,
            &file,
        )?;
        Ok(Self {
            file,
            parent,
            kind: CacheCustodyKind::LocalSnapshot,
            path: path.to_path_buf(),
            lock_name,
        })
    }

    pub(in crate::resolution::source) fn parent(&self) -> &CapabilityDirectory {
        &self.parent
    }

    pub(in crate::resolution::source) fn verify_path_identity(
        &self,
    ) -> Result<(), SourceResolveError> {
        verify_cache_lock_path_identity(
            self.kind,
            &self.path,
            &self.parent,
            &self.lock_name,
            &self.file,
        )
    }
}

fn local_snapshot_lock_timed_out(path: &Path, timeout: Duration) -> SourceResolveError {
    SourceResolveError::LocalSnapshotLockTimedOut {
        path: path.to_path_buf(),
        timeout_millis: u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX),
    }
}

pub(in crate::resolution::source) fn verify_cache_lock_path_identity(
    kind: CacheCustodyKind,
    path: &Path,
    parent: &CapabilityDirectory,
    lock_name: &OsStr,
    file: &File,
) -> Result<(), SourceResolveError> {
    let path_metadata = parent
        .symlink_metadata(lock_name)
        .map_err(|error| io_error(path, error))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock was replaced while being acquired",
        ));
    }
    let handle_metadata = file.metadata().map_err(|error| io_error(path, error))?;
    if !handle_metadata.is_file()
        || !same_std_and_capability_file_identity(&handle_metadata, &path_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache lock path does not identify the locked file",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &path_metadata)?;
    verify_macos_open_cache_extended_acl_custody(kind, path, file)?;
    verify_windows_open_cache_custody(kind, path, file)?;

    let parent_path = path
        .parent()
        .ok_or_else(|| cache_custody_invalid(kind, path, "cache lock has no publication parent"))?;
    verify_retained_cache_parent_path(kind, parent_path, parent)
}

pub(in crate::resolution::source) fn verify_retained_cache_parent_path(
    kind: CacheCustodyKind,
    parent_path: &Path,
    retained_parent: &CapabilityDirectory,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent_path, kind)?;
    let current_parent = open_absolute_directory_nofollow(parent_path)
        .map_err(|error| cache_custody_invalid(kind, parent_path, error.to_string()))?;
    let retained_metadata = retained_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    let current_metadata = current_parent
        .dir_metadata()
        .map_err(|error| io_error(parent_path, error))?;
    if !same_capability_file_identity(&retained_metadata, &current_metadata) {
        return Err(cache_custody_invalid(
            kind,
            parent_path,
            "cache parent pathname no longer identifies the retained directory",
        ));
    }
    Ok(())
}

pub(in crate::resolution::source) fn same_std_and_capability_file_identity(
    left: &std::fs::Metadata,
    right: &CapabilityMetadata,
) -> bool {
    use cap_fs_ext::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(test)]
pub(in crate::resolution::source) fn verify_cache_lock_path_identity_for_test(
    kind: CacheCustodyKind,
    path: &Path,
    file: &File,
) -> Result<(), SourceResolveError> {
    let parent_path = path.parent().expect("test cache lock has a parent");
    let canonical_parent = parent_path
        .canonicalize()
        .map_err(|error| io_error(parent_path, error))?;
    let lock_name = path.file_name().expect("test cache lock has a name");
    let canonical_path = canonical_parent.join(lock_name);
    let parent = open_absolute_directory_nofollow(&canonical_parent)
        .map_err(|error| io_error(&canonical_parent, error))?;
    verify_cache_lock_path_identity(kind, &canonical_path, &parent, lock_name, file)
}

impl Drop for CacheEntryLock {
    fn drop(&mut self) {
        // Keep the inode in place: unlinking a lock file lets a waiter lock the old inode while a
        // newcomer locks a replacement. Closing this handle releases the advisory lock safely.
        let _ = self.file.unlock();
    }
}

pub(in crate::resolution::source) struct PendingCacheEntry {
    pub(in crate::resolution::source) root: PathBuf,
    pub(in crate::resolution::source) parent: CapabilityDirectory,
    pub(in crate::resolution::source) directory: Option<CapabilityDirectory>,
    pub(in crate::resolution::source) stage_name: OsString,
    pub(in crate::resolution::source) published: bool,
}

impl PendingCacheEntry {
    pub(in crate::resolution::source) fn create(
        cache_dir: &Path,
        cache_directory: &CapabilityDirectory,
        cache_identity: &str,
    ) -> Result<Self, SourceResolveError> {
        let parent = cache_directory
            .try_clone()
            .map_err(|error| io_error(cache_dir, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!(
                ".git-{cache_identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            let root = cache_dir.join(&stage_name);
            match create_private_cache_directory(&parent, &stage_name) {
                Ok(()) => {
                    let provisional = ProvisionalCacheDirectory::new(&parent, &stage_name);
                    let directory = retain_private_cache_directory(
                        CacheCustodyKind::Git,
                        &parent,
                        &stage_name,
                        &root,
                    )?;
                    provisional.disarm();
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_invalid(
            cache_dir,
            "could not allocate a unique Git cache staging directory",
        ))
    }

    pub(in crate::resolution::source) fn directory(
        &self,
    ) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory
            .as_ref()
            .ok_or_else(|| cache_invalid(&self.root, "Git cache stage handle is absent"))
    }

    pub(in crate::resolution::source) fn create_private_directory(
        &self,
        name: &str,
        path: &Path,
    ) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        create_private_cache_directory(directory, name).map_err(|error| io_error(path, error))?;
        let provisional = ProvisionalCacheDirectory::new(directory, OsStr::new(name));
        retain_private_cache_directory(CacheCustodyKind::Git, directory, OsStr::new(name), path)?;
        provisional.disarm();
        Ok(())
    }

    pub(in crate::resolution::source) fn verify_path_identity(
        &self,
    ) -> Result<CapabilityMetadata, SourceResolveError> {
        let retained = self
            .directory()?
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_invalid(
                &self.root,
                "Git cache stage pathname no longer identifies the retained directory",
            ));
        }
        Ok(retained)
    }

    fn verify_parent_path_identity(&self, cache_dir: &Path) -> Result<(), SourceResolveError> {
        verify_retained_cache_parent_path(CacheCustodyKind::Git, cache_dir, &self.parent)
    }

    pub(in crate::resolution::source) fn verify_ambient_path_identity(
        &self,
        cache_dir: &Path,
    ) -> Result<(), SourceResolveError> {
        self.verify_parent_path_identity(cache_dir)?;
        self.verify_path_identity().map(|_| ())
    }

    pub(in crate::resolution::source) fn publish(
        &mut self,
        cache_dir: &Path,
        entry_root: &Path,
        entry_name: &OsStr,
    ) -> Result<(), SourceResolveError> {
        let retained = self.verify_path_identity()?;
        publish_cache_directory_from_open_parent(
            CacheCustodyKind::Git,
            cache_dir,
            &self.parent,
            &self.stage_name,
            entry_name,
            Some(&retained),
        )?;
        let published = self
            .parent
            .symlink_metadata(entry_name)
            .map_err(|error| io_error(entry_root, error))?;
        if !same_capability_file_identity(&retained, &published) {
            return Err(cache_invalid(
                entry_root,
                "published Git cache entry does not identify the retained stage",
            ));
        }
        self.published = true;
        Ok(())
    }
}

pub(in crate::resolution::source) struct ProvisionalCacheDirectory<'a> {
    pub(in crate::resolution::source) parent: &'a CapabilityDirectory,
    pub(in crate::resolution::source) name: &'a OsStr,
    pub(in crate::resolution::source) armed: bool,
}

impl<'a> ProvisionalCacheDirectory<'a> {
    pub(in crate::resolution::source) fn new(
        parent: &'a CapabilityDirectory,
        name: &'a OsStr,
    ) -> Self {
        Self {
            parent,
            name,
            armed: true,
        }
    }

    pub(in crate::resolution::source) fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for ProvisionalCacheDirectory<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.parent.remove_dir_all(self.name);
        }
    }
}

impl Drop for PendingCacheEntry {
    fn drop(&mut self) {
        if !self.published
            && let Some(directory) = self.directory.take()
        {
            make_open_tree_owner_writable(&directory);
            let _ = directory.remove_open_dir_all();
        }
    }
}

pub(in crate::resolution::source) fn create_private_cache_directory(
    parent: &CapabilityDirectory,
    name: impl AsRef<Path>,
) -> std::io::Result<()> {
    #[cfg(not(target_os = "wasi"))]
    {
        let mut builder = CapabilityDirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        parent.create_dir_with(name, &builder)
    }
    #[cfg(target_os = "wasi")]
    {
        parent.create_dir(name)
    }
}

pub(in crate::resolution::source) fn retain_private_cache_directory(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_custody_invalid(kind, path, error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
    }
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "private cache directory changed while being retained",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &opened)?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if opened.mode() & 0o777 != 0o700 {
            return Err(cache_custody_invalid(
                kind,
                path,
                "private cache directory does not have exact mode 0700",
            ));
        }
    }
    verify_macos_open_cache_extended_acl_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )?;
    verify_windows_open_cache_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )?;
    Ok(directory)
}
