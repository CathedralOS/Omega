//! Filesystem custody shared by local snapshots and Git cache entries.

use super::lock::same_std_and_capability_file_identity;
#[cfg(windows)]
use super::platform::verify_windows_open_cache_ancestry_identity;
use super::platform::{
    same_capability_file_identity, verify_cache_node_owner_and_mode,
    verify_capability_cache_node_owner_and_mode, verify_macos_cache_ancestor_acl_custody,
    verify_macos_cache_link_extended_acl_custody, verify_macos_open_cache_directory_acl_custody,
    verify_macos_open_cache_extended_acl_custody, verify_macos_open_cache_regular_file_acl_custody,
    verify_macos_opened_cache_directory_acl_custody, verify_windows_open_cache_directory_identity,
    verify_windows_open_cache_link_identity, verify_windows_open_cache_regular_file_identity,
};
use crate::SourceResolveError;
use crate::error::{cache_invalid, local_snapshot_invalid};
use crate::limits::{
    CACHE_CUSTODY_DEPTH_LIMIT, CACHE_CUSTODY_ENTRY_LIMIT, CACHE_CUSTODY_FIXED_BYTE_ALLOWANCE,
    GIT_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT, LOCAL_CACHE_CUSTODY_ABSOLUTE_BYTE_LIMIT,
    LocalSourceLimits,
};
use crate::tree::filesystem::{io_error, open_absolute_directory_nofollow};
use cap_fs_ext::DirExt;
use cap_std::fs::{Dir as CapabilityDirectory, Metadata as CapabilityMetadata};
use platform_custody::record_file::{RecordFileLimits, RecordFileRoot};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub(crate) enum CacheCustodyKind {
    Git,
    LocalSnapshot,
}

/// Exact bounded measurements produced by one complete capability-rooted
/// custody walk. This records accepted resident state after a helper exits; it
/// is not a during-write storage quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheCustodyMeasurement {
    pub(crate) entry_count: usize,
    pub(crate) logical_bytes: u64,
    pub(crate) maximum_depth: usize,
}

/// Retain one child of a parent whose ancestry the caller already verified.
/// The caller rechecks that retained parent's pathname after its bounded walk.
pub(crate) fn retain_cache_child_directory(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            path,
            "cache child is not a concrete directory",
        ));
    }
    let directory = open_cache_custody_directory(parent, Path::new(name), path, &classified, kind)?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    verify_capability_cache_node_owner_and_mode(kind, path, &opened)?;
    verify_macos_opened_cache_directory_acl_custody(kind, path, &directory)?;
    Ok(directory)
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
) -> Result<CacheCustodyMeasurement, SourceResolveError> {
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
    .map(|_| ())
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
    verify_windows_open_cache_directory_identity(kind, root, &metadata)
}

/// `verify_cache_custody_root` for a directory the resolver already retains,
/// plus the check that `root` still names that retained directory.
///
/// One no-follow open serves every fact about `root`: it must be the directory
/// just classified, its ACL facts are read through it, and it must be the
/// retained directory. Opening once for the ACL and again for the retained
/// comparison would let a replacement land between the two, so the ACL and
/// the identity would describe different objects; one handle binds all three.
pub(crate) fn verify_retained_cache_custody_root(
    root: &Path,
    kind: CacheCustodyKind,
    retained: &CapabilityDirectory,
    replaced: &'static str,
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
    let named = open_absolute_directory_nofollow(root).map_err(|error| {
        cache_custody_invalid(
            kind,
            root,
            format!("could not retain cache custody directory: {error}"),
        )
    })?;
    let opened = named
        .dir_metadata()
        .map_err(|error| io_error(root, error))?;
    if !opened.is_dir() || !same_std_and_capability_file_identity(&metadata, &opened) {
        return Err(cache_custody_invalid(
            kind,
            root,
            "cache custody directory changed between classification and no-follow open",
        ));
    }
    verify_macos_opened_cache_directory_acl_custody(kind, root, &named)?;
    let retained_metadata = retained
        .dir_metadata()
        .map_err(|error| io_error(root, error))?;
    if !same_capability_file_identity(&retained_metadata, &opened) {
        return Err(cache_custody_invalid(kind, root, replaced));
    }
    Ok(())
}

/// Check every directory above `root`, from the filesystem root down. `root`
/// itself is left to its caller, which applies a stricter owner and mode check
/// and its own descriptor-bound ACL check immediately afterwards. On macOS
/// each ancestor is opened from its verified parent's handle, so one walk
/// opens each ancestor once instead of reopening every prefix from `/`.
#[cfg(unix)]
fn verify_cache_ancestry(kind: CacheCustodyKind, root: &Path) -> Result<(), SourceResolveError> {
    use std::os::unix::fs::MetadataExt;

    let effective_user = nix::unistd::Uid::effective().as_raw();
    let ancestors: Vec<&Path> = root.ancestors().skip(1).collect();
    let mut verified_parent: Option<CapabilityDirectory> = None;
    for ancestor in ancestors.into_iter().rev() {
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
        verified_parent = verify_macos_cache_ancestor_acl_custody(
            kind,
            ancestor,
            &metadata,
            verified_parent.as_ref(),
        )?;
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
        verify_windows_open_cache_ancestry_identity(kind, ancestor, &metadata)?;
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
) -> Result<CacheCustodyMeasurement, SourceResolveError> {
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
) -> Result<CacheCustodyMeasurement, SourceResolveError> {
    let root_metadata = root_directory
        .dir_metadata()
        .map_err(|error| io_error(root, error))?;
    let mut pending = vec![(PathBuf::new(), root.to_path_buf(), root_metadata, 0usize)];
    let mut observed = 0usize;
    let mut logical_bytes = 0u64;
    let mut maximum_depth = 0usize;
    while let Some((relative_path, path, classified, depth)) = pending.pop() {
        maximum_depth = maximum_depth.max(depth);
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
                verify_windows_open_cache_regular_file_identity(
                    kind,
                    &child_path,
                    &directory,
                    &name,
                    &metadata,
                )?;
            } else if file_type.is_symlink() {
                verify_macos_cache_link_extended_acl_custody(kind, &child_path)?;
                verify_windows_open_cache_link_identity(
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
    Ok(CacheCustodyMeasurement {
        entry_count: observed,
        logical_bytes,
        maximum_depth,
    })
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
