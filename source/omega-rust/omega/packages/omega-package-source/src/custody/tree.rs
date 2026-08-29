//! Filesystem custody shared by local snapshots and Git cache entries.

#[cfg(windows)]
use super::platform::verify_windows_open_cache_ancestry_custody;
use super::platform::{
    same_capability_file_identity, verify_cache_node_owner_and_mode,
    verify_capability_cache_node_owner_and_mode, verify_macos_cache_link_extended_acl_custody,
    verify_macos_open_cache_directory_acl_custody, verify_macos_open_cache_extended_acl_custody,
    verify_macos_open_cache_regular_file_acl_custody, verify_windows_open_cache_custody,
    verify_windows_open_cache_directory_custody, verify_windows_open_cache_link_custody,
    verify_windows_open_cache_regular_file_custody,
};
use crate::SourceResolveError;
use crate::git::cache::identity::{cache_invalid, local_snapshot_invalid};
use crate::limits::{
    CACHE_CUSTODY_DEPTH_LIMIT, CACHE_CUSTODY_ENTRY_LIMIT, CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE,
    GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT, LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT,
    LocalSourceLimits,
};
use crate::local::capture::{io_error, open_absolute_directory_nofollow};
use cap_fs_ext::DirExt;
use cap_std::fs::{Dir as CapabilityDirectory, Metadata as CapabilityMetadata};
use omega_platform_custody::record_file::{RecordFileLimits, RecordFileRoot};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CacheCustodyKind {
    Git,
    LocalSnapshot,
}

pub(crate) fn read_bounded_cache_record(
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

pub(crate) fn read_bounded_cache_record_from_open_directory(
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

pub(crate) fn verify_git_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::Git,
        git_cache_custody_byte_limit(limits),
    )
}

pub(crate) fn verify_git_cache_root_custody(root: &Path) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::Git)
}

pub(crate) fn verify_local_cache_custody(
    root: &Path,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    verify_cache_custody(
        root,
        CacheCustodyKind::LocalSnapshot,
        local_cache_custody_byte_limit(limits),
    )
}

#[cfg(test)]
pub(crate) fn verify_local_cache_root_custody(root: &Path) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, CacheCustodyKind::LocalSnapshot)
}

pub(crate) fn verify_cache_custody_root(
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

pub(crate) fn git_cache_custody_byte_limit(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_mul(3)
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

pub(crate) fn local_cache_custody_byte_limit(limits: LocalSourceLimits) -> u64 {
    limits
        .max_bytes
        .saturating_add(CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE)
        .min(LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT)
}

pub(crate) fn verify_cache_custody(
    root: &Path,
    kind: CacheCustodyKind,
    byte_limit: u64,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(root, kind)?;
    let root_directory = open_absolute_directory_nofollow(root)
        .map_err(|error| cache_custody_invalid(kind, root, error.to_string()))?;
    verify_cache_custody_from_open_root(root, root_directory, kind, byte_limit)
}

pub(crate) fn verify_cache_custody_from_open_root(
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

pub(crate) fn cache_custody_has_capacity(observed: usize, pending: usize) -> bool {
    observed
        .checked_add(pending)
        .is_some_and(|retained| retained < CACHE_CUSTODY_ENTRY_LIMIT)
}

pub(crate) fn open_cache_custody_directory(
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

pub(crate) fn cache_custody_invalid(
    kind: CacheCustodyKind,
    path: &Path,
    message: impl Into<String>,
) -> SourceResolveError {
    match kind {
        CacheCustodyKind::Git => cache_invalid(path, message),
        CacheCustodyKind::LocalSnapshot => local_snapshot_invalid(path, message),
    }
}
